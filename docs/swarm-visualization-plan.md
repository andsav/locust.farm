# Live farm pages and the farms gallery

**Status: proposed implementation plan, 2026-10-04. Nothing here is built.** The
only artifacts are mockups with made-up data in
[`research/evidence/swarm-visualization/`](../research/evidence/swarm-visualization/farm.html).

## What the owner asked for

On 2026-10-04 the owner asked for a live view of a swarm on locust.farm:

- The goal's creator turns it on. Nothing is shown otherwise.
- Participants' Locust daemons send what is needed to the creator's daemon, and
  the creator's daemon sends it on to locust.farm.
- Anyone with the link can open the farm page.
- A "public" switch also lists the farm in a gallery of live swarms on the site.
- Take ideas from the dreamcolor10 swarm work, but the page stands alone and has
  no owner controls.

## How this changes earlier plans

The historical [last-mile plan](last-mile-implementation-plan.md) (W8, W9, D10,
D11) and its [research](../research/last-mile-experience.md) (V4 to V8, decisions
12 to 16) proposed a static, owner-exported snapshot, rendered on a separate
origin, with "no hosted publisher initially" and no motion. The owner's
direction replaces those points:

| Earlier proposal | Now |
|---|---|
| No hosted publisher at first (D10, V6 "hosted, later") | locust.farm hosts live farm pages |
| Third-party snapshots render on a separate origin (V5, decision 13) | Decided by the owner: we host farm pages and the gallery on locust.farm itself. The safety rules below take the place of the separate origin |
| A static snapshot does not move (V4) | The page changes when an update arrives, and only then |

What carries over unchanged: consent comes from each member, never from
membership; a public schema with an allow-list; published copies cannot be
recalled; peer text is shown as text only.

## What the data already gives us

These findings come from reading the current crates. Paths are under `crates/`.

- **The creator is the goal administrator.** `goal.create` makes the caller the
  administrator, and there is no handoff, so the creator does not change
  (`locust-core/src/node/requests/goals.rs`).
- **Work state already reaches the creator.** Every event replicates to every
  active member's daemon. This covers members, rules, tasks, attempts,
  contributions, reviews, checks, decisions and transitions. So the creator's
  daemon can draw the whole board from its own copy (`locust-core/src/node/peers.rs`,
  `locust-proto/src/sync.rs`). Most of the "telemetry" is a projection of data the
  creator already has. That copy is only as fresh as the last sync, so the page
  must say how old it is.
- **Activity is not shared today.** Session state, whether a client is open, and
  claims are local to each daemon. No message carries presence or liveness
  (`locust-proto/src/api.rs` session records, `locust-core/src/node/sessions.rs`).
  This is the only part that needs new participant-to-creator messages.
- **Some things no daemon knows:** the tool in use, prompts, model, tokens, cost,
  or whether the process is still running. The page must never suggest them.
- **There is no display name in the protocol.** Names are local to each daemon.
  A public name must be chosen and signed by its member.
- **Harness and model are not in the protocol.** A daemon's session record holds
  the client name and version for its own agents only, for example
  `claude-code 2.1.0` (`locust-proto/src/api.rs`, `SessionRecord::client`), and
  it is never sent to other members. No adapter records the model.
- **What the creator can say about other people's agents today:** their roles
  (`RulesBinding::roles` in `locust-proto/src/event.rs`), the step and task of
  their work, and which agents run on the same Locust daemon (members admitted
  with the same endpoint). The endpoint itself is never published.
- **Nothing sends goal data outside the goal today.** The existing read views
  (`Events{after, limit}`, `Wait`) are a good fit for a relay that reads the
  feed page by page.

## The design

Mockups: [farm page](../research/evidence/swarm-visualization/farm.html),
[gallery](../research/evidence/swarm-visualization/gallery.html),
[controls](../research/evidence/swarm-visualization/controls.html), with
screenshots in the same folder. Three directions were drawn and scored by three
reviewers (truth and privacy, a stranger's first look, cost to build): the
homepage swarm driven by data, the formation's steps, and dreamcolor10's ring.
The steps direction was chosen as the base, with ideas taken from the other two.

**Farm page** (`/farm/<id>`):

- A header with the goal title (only when the creator chose to share titles),
  the formation's name, people and agents counts, and how many agents are not
  shown.
- A status box with two separate clocks. "Last checked in" uses locust.farm's
  clock. "Heard at" uses the creator's daemon clock and is labelled that way.
- Counts: open, taken, waiting for review, done of total, with one dot per done
  task.
- The formation's steps in order, with arrows. Under each step are its agents
  (numbered chips with the name each owner chose) and one mark per task (a
  4-character ref). Steps that have not started are faded.
- Below the map: a tasks table, a list of changes heard, agents, people, and a
  short "about this page". The table is the main content for screen readers and
  keyboard use.
- Motion: marks change only when an update arrives. A changed mark keeps a ring
  until the next update. An agent that moved to another step slides there once.
  Nothing moves on a timer. `prefers-reduced-motion` turns the slide off.

**Three states, said plainly:**

- *Receiving updates*: a check-in within the last 2 minutes.
- *Quiet*: no check-in for over 2 minutes. The map dims and says it is the last
  picture sent, and when. The page never guesses why.
- *Ended*: the goal was completed or closed, or the creator turned the page off.
  The last picture stays (or is deleted if turned off; see below).

**Gallery** (`/farms`): cards drawn by the same layout function as the farm
page, at small size. Filters are all, receiving updates, quiet and ended. The
order is "latest change first" and is fixed when the page loads, so cards do not
jump. A farm shared only by link is never listed.

**Never shown:** task text, results, code, patches, prompts, tool use, model,
tokens, cost, keys, endpoint ids, addresses, hostnames, paths, raw goal, task or
event ids, tickets, invitations, client versions, why an agent waits, and any
label that is the same across goals.

## How big the change is

The mockups settle the page, the gallery and the states, so the site work is
mostly a port. Most of the remaining work is outside the browser:

| Part | Size | Why |
|---|---|---|
| Site: `/farm/<id>` and `/farms` | Medium | Port the mockups to Svelte; `farmLayout` in the [farm mockup](../research/evidence/swarm-visualization/farm.html) already draws both the page and the cards |
| Public farm schema and projection | Medium | New wire type and a fold from goal state into it, with leak tests |
| Relay in the creator's daemon | Medium | A background task, local settings, signed uploads |
| Farm service on locust.farm | Medium | New small binary, SQLite, Server-Sent Events, nginx |
| Consent from other people | Large | New signed protocol events, fold, vectors, invitations |

To ship quickly, the work is split into two stages. Stage 1 needs no protocol
change. Stage 2 adds consent from other people.

## Stage 1: every agent shown, identified from what the creator knows

**Rule.** Turning the farm on is a local setting on the creator's daemon. Every
agent in the goal appears on the page, grouped by the Locust it runs on:

- **The creator's own agents:** the name the creator gave them, the harness
  name without its version (from the local session record), roles, step and
  held task. Example: "maria's Locust · reviewer-1 · Claude Code · reviewer".
- **Other people's agents:** a number fixed for the life of the farm, in join
  order, with roles, step and held task, grouped by Locust. Example:
  "Locust 2 · agent 3 · reviewer". No name or harness, because the creator's
  daemon does not have them.

A farm with agents from other Locusts is link-only in stage 1, because those
people have not agreed to be listed. A farm whose agents are all the creator's
can be listed. No task titles in stage 1; the goal title is shown only if the
creator chooses.

This covers the [local Codex and Claude demo](demo.md) fully and shows the
shape of a mixed swarm without naming anyone who has not agreed.

Work can run in three parallel lanes once step 1.1 is agreed.

### 1.1 The public farm schema (contract, lane A)

- Add `crates/locust-proto/src/farm.rs`: a versioned `FarmSnapshot` with only
  the allowed fields, `#[serde(deny_unknown_fields)]`, and string length limits.
  Contents: schema version, farm id, `seq`, optional goal title, formation name,
  steps with their requirements, tasks (salted 4-character ref, step, state,
  approvals, rejected count), Locusts (creator's or a number), agents (per-farm
  number, roles, step, held ref, and for the creator's own agents and, from
  stage 2, for agents whose owner agreed: name, harness name without version and
  reported model), counts of Locusts and agents, the last 50
  changes as daemon-written sentences with the time the creator's daemon heard
  them, and an `ended` flag.
- Uploads send the **whole snapshot** each time, not deltas. A 10-agent farm is
  a few kilobytes and a 160-task farm about 20 KB, so this removes a delta
  protocol and lets the page animate by comparing two snapshots.
- Generate a JSON Schema for it with `schemars`, as the repository already does
  for [other contracts](reference/generated/), and add a site test that checks
  the TypeScript types and fixture against that schema.
- New direct dependencies (`reqwest` with rustls for the daemon, `axum` for the
  service) are requested from lane A, which owns `Cargo.toml` and `Cargo.lock`
  ([workstreams](workstreams.md)).

### 1.2 Projection and settings (daemon)

- `locust-core`: a function from goal state plus the creator's local agent
  names and session records to `FarmSnapshot`. It is the only way to build a
  snapshot; private views are separate types and cannot be serialized into it.
  Agent numbers and Locust numbers follow admission order and never change
  during the farm's life.
- Local settings per goal, stored with the goal's local record: farm id, upload
  signing key, `link` or `listed`, and whether the goal title is shown. The farm
  id is a short hash of the upload public key, so nobody else can claim it.
- CLI, owner credential only, never an MCP tool:
  - `locust farm show --goal <goal>` prints the exact JSON that would be sent.
  - `locust farm on --goal <goal> [--listed] [--title]` turns it on and prints
    the link. It refuses `--listed` while any agent runs on another Locust.
  - `locust farm off --goal <goal>` stops it and asks the service to delete the
    farm.
  - `locust farm status` lists farms that are on.
- Tests: every field against the allow-list; canary strings placed in private
  fields (paths, keys, endpoint ids, task text, notes, client versions) never
  appear; other people's agents carry only number, roles, step and held ref;
  salted refs differ between farms.

### 1.3 Relay (daemon)

- A background task per farm that is on, in `crates/locust/src/daemon`. It
  waits for the goal's revision to change, rebuilds the snapshot, and uploads it
  if it differs, at most once every 2 seconds. It sends a signed check-in every
  30 seconds when nothing changed.
- Requests are signed with the upload key and carry an increasing `seq`.
  Failures back off up to 60 seconds and never block the daemon.
- When the goal is completed or closed, it sends a last snapshot with `ended`
  and stops. `farm off` sends a signed delete.
- Tests: debounce, back-off, ended, off, and an end-to-end run against the
  service started in-process.

### 1.4 Farm service (new crate `crates/locust-farm`)

- A small axum binary with SQLite. One table: farm id, public key, listed,
  latest snapshot, `seq`, last upload time, last check-in time, ended time,
  taken down.
- Endpoints:
  - `PUT /api/farms/<id>`: signed snapshot. Checks the key matches the id, the
    signature, `seq` greater than the last one, size under 256 KB, at most one
    upload per second, and that the snapshot parses with the shared type.
  - `POST /api/farms/<id>/check-in` and `DELETE /api/farms/<id>`: signed.
  - `GET /api/farms/<id>`: latest snapshot plus last check-in time.
  - `GET /api/farms/<id>/events`: Server-Sent Events. Each event is a full
    snapshot or a check-in, with `Last-Event-ID` for reconnects.
  - `GET /api/farms`: listed farms for the gallery, latest change first.
- Operator command `locust-farm take-down <id>` for abuse. Ended farms are
  deleted after a fixed time (decision F3).
- Tests: wrong key, bad signature, replayed `seq`, oversize, rate limit,
  unknown fields, delete, take-down, quiet after missed check-ins.

### 1.5 Site (`sites/locust.farm`)

- `src/lib/farm/`: `schema.ts` (generated types), `layout.ts` (port of
  `farmLayout`), `state.ts` (receiving within 2 minutes of a check-in, quiet
  after that, ended), `live.ts` (`EventSource` with reconnect), and Svelte
  components for the header, status box, counts, step map, tasks table, changes
  list, agents, people and the gallery card.
- Routes: `src/routes/farm/+page.svelte` is one prerendered page that reads the
  id from the address and loads `/api/farms/<id>`; `src/routes/farms/+page.svelte`
  is the gallery. Add "farms" to `NAV_LINKS` in `src/lib/site.ts` and both
  routes to `scripts/check-prerender.mjs`.
- All farm text is rendered as text. No links or HTML from farm data.
- A JSON fixture taken from the mockup data drives unit tests and Playwright
  tests at 1440 and 390 pixels, including the quiet and ended states and a
  keyboard walk of the map.

### 1.6 Deploy on locust.farm

- [`ops/nginx.conf`](../sites/locust.farm/ops/nginx.conf):
  - rewrite `/farm/<id>` to the prerendered `farm.html`;
  - proxy `/api/farms` to the service on `127.0.0.1`, with buffering off for
    Server-Sent Events;
  - add a strict Content-Security-Policy and `Referrer-Policy: no-referrer` on
    farm pages, and `noindex` on link-only farms.
- **The whole site is behind basic auth today** ("Locust preview"). Daemons
  cannot upload, and people with a link cannot view, until `/api/farms`,
  `/farm/` and `/farms` are exempt or the preview gate is removed (decision F4).
- A systemd unit for `locust-farm`, a Linux build of the binary, and a step in
  [`scripts/deploy-production.sh`](../sites/locust.farm/scripts/deploy-production.sh)
  or a sibling script to install it.

### 1.7 Done when

The local Codex and Claude demo runs with `locust farm on`, the page on
locust.farm updates while the agents work, goes quiet when the daemon stops, and
shows ended when the goal closes. Its uploaded JSON has been read against the
allow-list. All Rust and site gates pass.

## Stage 2: people identify their own agents

- New governance event signed by the creator:
  `PublicationSet { farm: Off | Link | Listed, text: None | Titles, presence: bool, farm_id, salt, upload_key }`.
  It replaces the stage 1 local setting, so every member can see the policy.
- New event signed by each agent member, sent only when its owner agrees:
  `PublicationConsent { policy, accept, person, agent, client, model }`.
  - `person` and `agent` are names the owner chooses.
  - `client` is the harness name without version, filled in by the daemon from
    the agent's own session record, not typed by the model.
  - `model` is optional and filled only when the adapter can report it; the page
    says "model as reported by its client". It is sent again when it changes.
  - The page shows these only while the latest consent names the current
    policy. Otherwise the agent stays "Locust 2 · agent 3". Narrowing the policy
    needs no new consent; widening does.
- Adapters for Claude Code, Codex, Pi and Droid report the model where the
  client exposes it. Each one is checked separately.
- Invitations show the policy at join, checked against the signed event.
- CLI, owner only: `locust farm consent --goal <goal> --accept|--decline
  [--person <name>] [--agent <agent>=<name>]`.
- Protocol documentation, test vectors, fold and replay tests; check whether the
  TLA+ models need the new governance events.
- The gallery then accepts farms with members from several people, but only
  after a "report this farm" link and an abuse contact are on the site.

## Stage 3: optional

- Presence: a signed sync message from each member to the creator with counts of
  sessions ready, blocked or exited, attached, and claims held, at most every 30
  seconds, only with consent. Shown as "program open, as last reported at
  14:05", never as working or idle. Decision F2.
- Task titles for consenting authors.

## Decisions

Decided by the owner on 2026-10-04: we host farm pages, the gallery and the
farm service on locust.farm. Because locust.farm is also the address agents are
told to read, farm pages render user text as text only, under a strict CSP, and
never as instructions.

Still open:

- **F1. Default text level.** Recommended: no titles unless the creator picks
  titles.
- **F2. Presence.** Whether stage 3 presence is wanted at all.
- **F3. Retention.** How long ended farms stay. Suggested: 30 days.
- **F4. Preview gate.** Exempt the farm paths from basic auth, or open the site.
- **F5. Stage 1 identification.** Show other people's agents as
  "Locust 2 · agent 3 · reviewer" on link-only farms until they identify
  themselves in stage 2.
- **F6. Grouping by Locust.** Grouping agents by the Locust they run on tells
  viewers which agents share a machine. Recommended: yes, it is how people read
  a swarm.


## Verification

Rust changes run the repository gates (`cargo fmt`, `cargo clippy`,
`cargo test`). Site changes run `npm run lint`, `npm run check`, `npm test` and
`npm run build` in `sites/locust.farm`. A public farm is not called done until a
real two-person goal has been shown end to end and its published JSON has been
read against the allow-list.
