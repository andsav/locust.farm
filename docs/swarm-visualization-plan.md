# Farm pages: design and demo plan

Status: first version built. Not deployed. The two-machine demo with real agents has not run.

A farm page is a public, read-only web page that shows one goal's progress. The
administrator's daemon uploads a snapshot to a farm service on locust.farm.
Anyone with the link can view the page; the owner may also list it in the
`/farms` gallery. The [farm guide](guide/farm-publication.md) explains how to
use it, and the [test report](../research/farm-qualification.md) has the
results.

The mockups of the [farm page](../research/evidence/swarm-visualization/farm.html),
[gallery](../research/evidence/swarm-visualization/gallery.html) and
[controls](../research/evidence/swarm-visualization/controls.html) use invented
data. They set the visual direction only.

## What is built

Current source only; the published preview does not include this.

- Snapshot schema, signed requests, policy and consent records:
  [locust-proto](../crates/locust-proto/src/farm.rs).
- Publisher (snapshots, consent checks, numbering, outbox):
  [locust-core](../crates/locust-core/src/node/farm.rs) and its
  [transport](../crates/locust/src/daemon/farm.rs).
- Owner commands `farm on`, `off`, `show`, `status` and `consent`:
  [CLI](../crates/locust/src/cli/farm.rs).
- Farm service with `serve`, `enroll` and `take-down`:
  [locust-farm](../crates/locust-farm/src/lib.rs).
- Pages `/farm/<id>` and `/farms`:
  [farm components](../sites/locust.farm/src/lib/farm/).
- nginx and systemd configuration: [operator guide](../sites/locust.farm/ops/README.md).
- Scripted two-daemon check: [check_farm.py](../scripts/check_farm.py).
- Demo formation: [team-chat.json](../examples/demos/team-chat.json).

## Decisions

Confirmed by the owner:

- The goal's creator turns publication on. Gallery listing is optional.
- Farm pages, the gallery and the service run on locust.farm.
- The demo uses two machines owned by one person, with several agents and
  different clients on one problem.

Built as defaults, not each confirmed by the owner:

- Consent and public profiles from the start, with no anonymous mode.
- Consent from every covered member, with no partial view.
- No task text, model names or presence.
- Only an explicit close decision ends a goal; ended farms are kept 30 days.
- Only farm pages, the gallery, `/api/farms` and their assets skip the preview
  password.
- Only farm IDs the operator enrolled may publish.

Fixed rules:

- Consent is explicit. Admission, shared ownership or a shared daemon never
  imply it.
- Nobody can recall published copies.
- Farm text is untrusted and shown as plain text. The CSP stops scripts, not
  prompt injection, so agents must never treat farm text as instructions.

## Public data contract

`FarmSnapshot` is the only public format; the site types are generated from
its schema. Every object rejects unknown fields, and the service checks lengths
and references. Only the daemon's farm code builds snapshots; private views
never go into them.

A snapshot never holds protocol IDs, keys, task text, results, code, prompts,
tool activity, models, costs, addresses, paths, tickets or client versions. It
holds these items:

- **Agent**: a member with an accepted public profile, not a process or a
  person. Several sessions stay one agent.
- **Daemon group**: the members admitted at one daemon. It does not prove a
  machine or owner. Its owner-reported label is dropped if its agents disagree.
- **Harness**: the agent's client, read from the daemon's session record at
  consent: `codex`, `claude_code`, `factory_droid`, `pi`, `unknown` or
  `multiple`.
- **Stage**: the task round's actual stage and prerequisites, labeled by the
  owner or "Stage N". Unstaged tasks stay unstaged.
- **Attempt**: the last reported state. Started does not mean a process runs.
- **Task**: its current round's state. One agent may work in several stages,
  and many agents may try one task.
- **Result**: a contribution, its requirement as fixed text, and the evidence
  count for that exact result.
- **Goal state**: open, ended, unavailable or disputed. An empty board or
  finished tasks do not end a goal.

Rules:

- Attempt completion is not task completion, which is not selection or local
  application.
- Reviews count per exact result, never pooled. A rejection is evidence, not a
  veto. Rules without a simple review count show their requirement, not an
  invented fraction.
- Task counts use one state per task, in this order: closed, completed,
  awaiting evidence, reported, open. A task closed unfinished is not completed.
  Failed and abandoned attempts stay visible.
- Revisions, removals and conflicting records can change an earlier outcome;
  the snapshot follows the daemon's current view and lists each retraction. Old
  rounds never lend approvals to a current task. A taskless contribution is
  listed only as a change.

### Identifiers, text and time

- Agents, groups, stages, tasks and results get per-farm numbers on first
  sight, initial members in admission order. Numbers are saved and never reused.
  A new farm gets new numbers.
- Task references are at least four hex characters from a salted hash, longer
  on collision. They are labels only and do not link farms.
- Owners type every public name, label and title; Locust copies none on its
  own. A name reused across farms is not anonymous.
- Change text comes from fixed templates. The recent-change window (default 50)
  states how many entries it left out.
- The creator records when it first sees each change; author timestamps are not
  arrival times. History from before the farm started has no observation time.
- Receipt, observation and per-goal peer sync times are shown separately. A
  fresh check-in can carry old peer data.

## Consent

- `farm on` makes the administrator sign a `PublicationSet` record: farm ID,
  upload public key, visibility and the disclosure policy (title, labels and
  recent-change count). The upload private key and reference salt stay local.
- `farm consent` signs a `PublicationConsent` record as one local agent: the
  policy digest, accept or decline, and a public profile (name, optional group
  label, harness). Each daemon's owner consents for its own agents.
- Only the local owner can run these commands; agents and MCP sessions cannot.
- Every active member must consent, and every author of work that still counts,
  including removed members.
- A changed policy needs everyone's consent again, and a changed profile a new
  consent. The creator cannot consent for a remote agent.
- Invitations carry the policy and say that joining is not consent.
- The daemon checks eligibility on every change and before each request. A new
  member without consent, a decline, a missing proof or a conflict makes the
  farm ineligible. The daemon then queues a signed suspend, and the service
  hides the page. The farm resumes under the same ID once eligible again.
- The old page may stay up until the service receives the suspend; `farm status`
  shows it as pending. A remote decline counts once it reaches the creator.

## Page and gallery

`/farm/<id>` shows the title, formation label, agent and group counts,
freshness, a stage map, a task table, recent changes and a note on what it
shows. It never infers a number of people.

- The task table is the full keyboard and screen-reader view. It must stay
  readable at 1440 and 390 pixels wide.
- A mark moves once per meaningful change, never for check-ins. The page
  respects `prefers-reduced-motion`.

Page states, by precedence:

1. **Unavailable.** Unknown, suspended, deleted or taken down. No reason is
   shown. An open page clears its snapshot; a disconnected one says so.
2. **Ended.** The goal is closed. A reopen resumes publishing.
3. **Receiving updates.** The service heard from the publisher within two
   minutes.
4. **Quiet.** Nothing for over two minutes. The last snapshot stays.

`/farms` lists listed, eligible farms with all, receiving, quiet and ended
filters, in pages, newest change first. Check-ins do not reorder cards.
Link-only farms never appear. Cards that become unavailable disappear without
reshuffling the rest.

## Publisher and service contract

### Publisher

- Only the creator's daemon publishes. It saves its keys, numbering,
  observation times, sequence and pending request.
- It builds the snapshot inside the engine's single writer and sends it from
  outside. Settings, consent and closure changes wake it too.
- It sends a whole snapshot when content changes, at most every two seconds, and
  a check-in every 30 seconds while the goal is open. Retries back off up to 60
  seconds and never block agent work.
- A snapshot that is too large fails with a local error; nothing is dropped
  silently.

### Signed requests

- Each request is signed over its version, operation, farm ID, sequence and a
  digest of the exact body. A
  [test vector](../crates/locust-proto/fixtures/farm-signature-vector.json)
  fixes the encoding.
- The farm ID is 128 bits of a hash of the upload public key. This stops others
  from changing a farm; it does not prevent spam.
- All requests share one increasing sequence. The daemon saves each request
  before sending and retries the same bytes. A repeat gets the earlier receipt;
  a different body or older sequence is refused.
- Off and suspend go ahead of queued uploads. The owner sees "pending" until the
  service confirms.
- Deletion, takedown and expiry leave a tombstone that refuses later uploads,
  clears the snapshot and closes open streams. A farm turned on again gets a
  new ID.

### Service

Endpoints: `PUT`, `GET` and `DELETE /api/farms/<id>`, `POST` to its
`check-in` and `suspend`, server-sent events at its `events`, and the gallery at
`GET /api/farms`. A viewer gets the current state on connect, then each new full
state; there is no event history.

Defaults: a 256 KiB body, one change per second per farm, 512 streams and 500
requests per second. Per-key limits do not stop mass key creation. A reopen
within 30 days clears the expiry; a quiet farm never gets an end time. The
operator has `take-down`, an abuse contact, and must see health, disk and
database errors.

### Hosting

nginx serves every `/farm/<id>` from one static page and proxies `/api/farms` to
the service without buffering. Farm responses set no-store, no-referrer and
noindex. This passed in a local nginx container; on locust.farm, `/api/farms`
still asks for the password.

## Two-machine demo

### Roster and problem

Four agents, a rehearsal choice, on two physical machines owned by one person.

| Machine | Client | Role |
| --- | --- | --- |
| A, publisher | Codex | Coordinator and integrator |
| A | Claude Code | Frontend |
| B | Codex | Backend |
| B | pi, or another client that passes preflight | Reviewer and tester |

The owner may change the roster. The minimum is several agents, two clients
and real work on both machines, never simulated. Record which roster ran.

The problem is a small team chat app like Slack: channels, a display-name
picker, history that survives a restart, and live messages between two browser
sessions. Use made-up content. Login, payments, uploads, search, threads and
hosting are out of scope. Keep the problem fixed during rehearsal.

### The team-chat formation

[team-chat.json](../examples/demos/team-chat.json) has four roles: coordinator,
frontend, backend and reviewer. A non-author reviews each result. The
coordinator selects results and finishes the goal. It passes
`locust formation validate`.

```text
contract -> frontend --\
         -> backend  ---> integration -> verification
```

- The contract stage sets the API, data model, layout, test and start commands,
  so frontend and backend can work in parallel.
- Each later stage needs the selected result of each earlier stage, not just a
  review.
- The daemon creates stage tasks from the formation, not from task titles.
- The administrator's daemon must keep running to create ready stages.

### How the agents work

- Each coding agent works in its own folder copied from the agreed snapshot.
  Findings, snapshots and signed patches travel through Locust.
- The integrator applies the selected patches, runs checks and publishes the
  combined result.
- The reviewer tests that exact result in a new folder. On failure, the right
  stage is revised; approval of an older result is never reused.
- Native sessions stay open and read pending work through their Locust tools.
  Check delivery or explicit resume for each client; do not assume automatic
  wake or hooks.
- The owner sets the goal and local permissions but never carries findings or
  patches between chats. Record any manual steering.

### Run steps

1. Install the same Locust build on both machines; the preview lacks farm
   commands. Record privately the commit, binary hashes, client versions,
   models and permissions. Check model access and tool discovery.
2. Create a new goal. Admit the roster before binding the rules and granting
   execution. Check that incomplete bindings create no work, and that records
   cross between machines.
3. Start each agent's session before it consents, so its harness is recorded.
   Turn the farm on, consent on both machines, and check `farm show`.
4. Open the farm without the preview password, next to the app. Show both
   machine groups and harness labels, then watch each stage arrive.
5. Show the app in two browser sessions: different names, live messages,
   channels, and history after a restart. Record a remote agent's contribution
   that changed the result; distinct agent chips alone prove nothing.
6. Rehearse recovery separately. Disconnect B while A publishes, then reconnect.
   Stop A's daemon to show quiet, then restart it with the same farm. Check for
   duplicate tasks or patches. Stopping a daemon does not stop native clients.
7. After checking the result, the owner closes the goal through the coordinator.
   Show ended. Separately test reopen and deletion, including deletion while
   offline and with a viewer open.

Measure the delay from a record to the browser over both hops; do not promise
one. The page never streams terminal output, reasoning or tool calls. Label any
recording or scripted run.

## Remaining work

| Work | Done when |
| --- | --- |
| Deployment of the service, nginx and systemd | A browser without credentials loads farm links and live updates; a restart and a deletion pass |
| Rehearsal on two physical machines | Identified builds, real contributions and reviews, a working app, measured delay, redacted evidence in `research/` |
| Formal models | Decide whether the [TLA+ models](../research/tla/README.md) should cover consent, the outbox, receipts and tombstones |

Tests must cover consent changes on either machine, new members, removed
authors, hostile text, parallel and unstaged work, missing times, stale peers,
retractions, reopen, collisions, bad signatures, replays, restarts with pending
requests, oversized snapshots, stream reconnects and takedown. Browser tests
cover keyboard use, reduced motion and every page state.

Keep scripted and real-model runs as separate records. The demo is done when
agents on two machines build a working app and the deployed farm shows it
correctly. Independent people and open hosting come later.

## Later extensions

Only if wanted, within the consent rules above: signed presence reports, model
labels tested per adapter, task titles with the author's consent, and partial
publication with its own redaction design.

## Open questions

- Should consent cover visibility? Today a link-only farm can become listed
  without new consent.
- Should members see the exact snapshot before consenting? Today `farm show`
  returns it only after the last consent, when upload starts.
- When will the service be deployed? Until then, `farm on` with the default
  service retries against HTTP 401.
- The site shows Droid agents as "Harness unknown", and `shell` agents have no
  harness value.
- Rehearsal inputs: host environments, installed clients, model access, the
  app's stack and the builds to use.
- A public enrollment and abuse policy, before open hosting.
