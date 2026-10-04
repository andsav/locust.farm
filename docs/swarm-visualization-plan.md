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

## Work packages

Each package is small enough to ship on its own. P1 can be shown before any
Rust work.

### P1. Farm page and gallery on the site, from a recorded file

- Add `/farm` and `/farms` routes to `sites/locust.farm`. The site uses
  `adapter-static` and is deployed by `scripts/deploy-production.sh` behind
  nginx. Keep it static: `/farm` is one prerendered page that reads the farm id
  from the path (an nginx rewrite to the page) and loads data with `fetch`.
- Define the public farm schema as TypeScript types plus a JSON fixture (the
  mockup's synthetic swarm). The page draws a snapshot and applies changes.
- Reuse `SiteHeader.svelte` (add "farms" to `NAV_LINKS` in `src/lib/site.ts`),
  the tokens, `onboarding/clipboard.ts` for Copy link, and the formation
  editor's decoding and step order. The old xyflow stage canvas is gone; draw
  the steps with plain SVG and HTML.
- Render all user text as text nodes. No HTML, no links built from user text.
- Tests: unit tests for the layout and the state rules (receiving, quiet,
  ended), Playwright tests at 1440 px and 390 px, keyboard walk of the map.

### P2. The public farm schema and projection in Rust

- A `locust-proto` type for the public farm, versioned, with only the allowed
  fields. Ids are export-local: a random `farm_id`, task refs as short salted
  hashes, people as salted handles, so a member cannot be matched across farms.
- A projection in `locust-core` from the goal state to that type. It is a
  separate type from private views so a private view cannot be serialized by
  mistake.
- `locust goal farm show --goal <goal>` prints the JSON the page would receive.
  This lets the creator see exactly what would be published.
- Tests: a forbidden-field test over every field, canary strings in private
  fields that must not appear, people without consent counted but not named.

### P3. Turning it on, and consent (protocol change)

- New governance event signed by the creator:
  `PublicationSet { farm: Off | Link | Listed, text: None | Titles, presence: bool, farm_id, salt, relay_key }`.
  It must be signed and replicated, because it moves the read boundary for
  content every member wrote.
- New event signed by each member: `PublicationConsent { policy, accept }`. A
  member's agents, name and presence are shown only while their latest consent
  names the current policy. Otherwise they appear only in counts as "not shown".
  Narrowing the policy needs no new consent; widening it does.
- Invitations show the policy at join, checked against the signed event when it
  arrives.
- CLI: `locust goal farm set --goal <goal> --show link|listed|off --text none|titles`
  and `locust goal farm consent --goal <goal> --accept|--decline`. Not exposed as
  model tools; the person answers. The agent can show the prompt and the
  command, as in the controls mockup.
- Update the protocol docs, test vectors and fold tests. Check whether the TLA+
  models need the new governance event.

### P4. The relay and the farm service

- The creator's daemon reads its own goal feed (`Events`, `Wait`), folds it with
  the P2 projection and uploads the snapshot and changes to locust.farm every 1
  to 5 seconds when something changed, plus a check-in every 30 seconds when
  nothing did.
- Uploads are HTTPS POSTs signed with the `relay_key` from the policy. The
  service checks the signature, a sequence number (no replays), size and rate
  limits.
- The farm service is a small Rust binary in this workspace (axum), running on
  the same server behind nginx, storing the latest snapshot and recent changes
  in SQLite. Viewers get the snapshot, then changes over Server-Sent Events with
  `Last-Event-ID` for reconnects.
- Headers on farm pages and the API: a strict Content-Security-Policy,
  `X-Robots-Tag: noindex` for link-only farms, `Referrer-Policy: no-referrer`.
- Turning the page off deletes the farm on the server and stops the relay. The
  page and the docs say copies already seen cannot be taken back.
- Tests: signature and replay rejection, limits, quiet after missed check-ins,
  delete on off, end-to-end from a local daemon to a local service to the page.

### P5. Presence from participants (optional, last)

- A new sync message, signed by the member and sent only to the creator: number
  of sessions ready, blocked or exited, number attached, active claims, client
  name without version, and when it was observed. Not written to the event log.
- Sent when it changes, at most every 30 seconds, only with consent and only if
  the policy has `presence: true`.
- The page shows it as "program open, as last reported at 14:05", never as
  "working" or "idle".

### P6. Gallery rules and operations

- Listing only for `Listed` farms with at least one consenting member.
- A "report this farm" link, an operator takedown command on the server, and an
  abuse contact on the site. Titles are user text, so takedown must exist
  before the gallery is public.
- Keep ended farms for a fixed time (for example 30 days), then delete.

## Order

P1 first (it can be shown with recorded data). P2 next, so the exact public data
can be reviewed before anything is sent. Then P3 and P4 together, because
nothing may be uploaded without the signed policy and consent. P5 and P6 last;
P6 must be done before the gallery lists anyone else's farm.

## Decisions

Decided by the owner on 2026-10-04: we host farm pages, the gallery and the
farm service on locust.farm. Because locust.farm is also the address agents are
told to read, farm pages render user text as text only, under a strict CSP, and
never as instructions.

Still open:

- **F1. Default text level.** Recommended: no titles unless the creator picks
  titles.
- **F2. Presence.** Whether P5 is wanted at all.
- **F3. Retention.** How long ended farms stay.

## Verification

Rust changes run the repository gates (`cargo fmt`, `cargo clippy`,
`cargo test`). Site changes run `npm run lint`, `npm run check`, `npm test` and
`npm run build` in `sites/locust.farm`. A public farm is not called done until a
real two-person goal has been shown end to end and its published JSON has been
read against the allow-list.
