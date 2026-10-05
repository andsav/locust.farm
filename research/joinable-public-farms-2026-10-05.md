# Joinable public farms: proposal

Status: proposal of 2026-10-05 at `6f42d84`. Nothing here is implemented, built
or run. Statements about today's code were read in the source. The ones the
design rests on are linked and were read twice, by an investigating agent and
again by the author. Costs given as numbers are arithmetic from constants and
two existing measurements, not new measurements.

The question was how a person can create a public farm that strangers join
easily, "using a QR code or equivalent", when the farm's goal is already in
progress.

## Method

Two passes of independent agents, then a check by the author.

1. Nine agents mapped what exists: invitations, membership and permissions,
   late-joiner catch-up, the farm service and consent, networking, onboarding,
   recorded decisions, and two bodies of prior art (how other systems join
   people by link or code, and how open-participation systems accept strangers'
   work).
2. A first design was written from that map. Seven agents then attacked it:
   four checked feasibility against the code (admission, farm service,
   formations, scale and networking), one acted as a red team, one walked the
   host's and joiner's journeys, and one argued for different architectures.
   All seven returned "needs changes". The design below includes their
   corrections; [what the review changed](#what-the-review-changed) lists them.
3. The author re-read the admission function, the farm eligibility function,
   the presets, the refusal type, the pre-admission connection budget, the farm
   upload body and the farm identifier.

Limits: no build, test or network request was made. Prior-art entries marked
"from memory" by their agent are not used as evidence here.

## Answer

The QR code carries the farm's address and nothing else. Joining stays a direct
handshake between the joiner's daemon and the host's daemon, which admits
automatically as it does today. Six things change around that.

- **A door instead of a ticket.** The host's daemon keeps one standing record
  that says joining is open, to how many people and until when. A signed
  descriptor of it rides the existing publisher channel to the farm service.
- **One reviewed command for the joiner.** `locust join <farm-address>` shows a
  plan and, after a yes, joins, records the public name and grants the joiner's
  own agent its permissions.
- **Consent can no longer take the page down.** On a joinable farm a member
  without a valid consent is shown as an unnamed participant.
- **A joinable goal is public from its creation.** Every member reads the whole
  goal, so a door opens only on a goal that has no earlier work.
- **Only a formation that keeps decisions with chosen members qualifies.** A new
  `public` preset is the only built-in one that does.
- **A small hard limit on members.** Sixteen in total, host included, until a
  larger number has been measured.

## What today's code does

Each of these shaped the design.

- **Admission is automatic and belongs to one daemon.** A join request reaches
  `plan_join` in [peers.rs](../crates/locust-core/src/node/peers.rs), which
  signs `MemberAdmitted` with the administrator's key when the request is valid.
  No person approves a joiner. Membership counts only on the administrator's own
  log ([chain.rs](../crates/locust-core/src/goal/chain.rs)), so no other member
  and no web service can admit.
- **Single use is one local field.** The issuer's record holds one `redeemed`
  pair; `plan_join` refuses any other key. The ticket, the join request and the
  sync frames carry no use count
  ([invitations.rs](../crates/locust-core/src/node/requests/invitations.rs),
  [invite.rs](../crates/locust-proto/src/invite.rs)).
- **Only one refusal is terminal for a joiner.** Its daemon gives up on a join
  only for `InvitationRefused`; every other ending is retried with backoff and
  no deadline ([sync.rs](../crates/locust-proto/src/sync.rs),
  [driver.rs](../crates/locust-core/src/sync/driver.rs)).
- **Every join suspends the public page.** `eligible` in
  [farm.rs](../crates/locust-core/src/node/farm.rs) requires a current,
  accepting consent from every active member and from every author of effective
  work, removed members included. A test pins that a joining member suspends the
  farm ([tests/farm.rs](../crates/locust-core/src/node/tests/farm.rs)).
- **Removing a member does not restore the page.** Removal keeps the member's
  accepted events effective, so a removed author who declined stays required.
- **A member reads everything.** The invitation's sharing boundary has one
  variant, the whole goal, and admission sets the read ceiling to the current
  key epoch ([sharing guide](../docs/guide/sharing.md)). There is no observer
  tier.
- **Permissions are local.** Joining grants nothing. Each daemon's own owner
  grants the seven per-goal permissions to its agents; the host cannot grant or
  see a remote member's permissions.
- **A ticket does not fit a QR code.** It is the hex of up to 4 KiB, includes
  the host's IP addresses, and by the agents' estimate runs to 700 to 1,300
  characters with a publication policy attached (not measured).
- **The public page deliberately says nothing about how to reach the goal.**
  `FarmUploadBody` is exactly a visibility and a snapshot and rejects unknown
  fields ([farm.rs](../crates/locust-proto/src/farm.rs)); tests assert that the
  goal id and keys never appear. The daemon only pushes to the service
  ([daemon/farm.rs](../crates/locust/src/daemon/farm.rs)).
- **Four of the six presets let a stranger finish tasks alone.** The default
  completion rule is a declaration by the result's own author
  ([organization.rs](../crates/locust-proto/src/organization.rs)).
  `independent-attempts` adds only a selection authority, so it inherits that
  rule ([presets.rs](../crates/locust-proto/src/organization/presets.rs)).
  `peer-review` and the pipeline's draft stage accept one approval from any
  other member, which a second key provides.
- **Seven strangers can hold every inbound slot.** The pool for unadmitted
  connections is 64 MiB divided by the per-connection receive credit, which is
  seven, each held for up to 30 seconds
  ([network.rs](../crates/locust/src/daemon/network.rs)).
- **Sync is a full mesh.** Every daemon pairs with every other active member.
  Nothing larger than three daemons on one machine has been run
  ([simulation notes](multi-machine-simulation.md)).
- **The joiner needs a Mac and a newer build.** The installer supports macOS on
  Apple Silicon only, and the published build speaks protocol 4 while source
  speaks 6 and has no `farm` command ([status](../docs/status.md)).

## Proposed design

### What joining means

Coming through the door makes a person's agent a full member of the goal the
page shows. It reads the whole goal and may write wherever the formation says
`members`. No guest or observer tier is built.

Because of that, a door opens only on a goal with no effective work yet: the
farm is turned on as joinable when it is created. A team with private history
makes a second, public goal and moves chosen material across by hand, which is
the pattern already qualified in
[subgroup-qualification.md](subgroup-qualification.md). Opening an existing
goal is refused in the first version.

### The public artifact

The QR code encodes the farm's address. Today that is
`https://locust.farm/farm/` plus 32 hex characters, 57 characters in all. It
holds no secret and no addresses and does not change while the farm lives.

A phone that scans it can only watch, because the phone has no daemon. The page
therefore also shows a short reference a person can type on their Mac. The farm
service would assign each farm an eight-character alias (vowel-free and
case-insensitive, as RFC 8628 recommends for typed codes) and redirect
`/f/<alias>` to the farm page. Pairing a phone with a daemon is left for later.

### The door

The door is one issuer-local record on the host's daemon with an explicit kind
(door, not invitation), a mode, a seat total, an expiry and a public door id
that never changes.

- The door id takes the place of the invitation secret. It is public by
  construction, so nothing rotates and nothing needs hiding from argv or chat.
- The descriptor is the existing signed `Invitation` with relay-only hints and
  the consented public title in place of the goal's own title. The daemon signs
  it again and uploads it whenever its relay address changes.
- Open, close, seats and expiry are edits to the local record. Only the fact
  that the goal permits joining enters signed history, as a field of the
  disclosure policy.
- Two modes share the record: `open` admits until seats run out; `ask` holds
  each request in a pending list until the host approves it.

### Admission

`plan_join` keeps signing `MemberAdmitted`. The changes:

- The idempotent answer runs first and keys on chain state: a key that is
  already an active member at the same endpoint gets Ok. Today a revoked record
  is refused before that branch, which would strand a joiner whose first answer
  was lost once doors can be closed after use.
- `MemberAdmitted` records how the member came in (invitation or door). The tag
  is under the administrator's signature and cannot be forged.
- Seats are consumed and never returned. The count is the number of door
  admissions on the administrator's own chain. A separate check refuses
  admission once active members reach the goal's ceiling.
- New refusals (door full, door closed, door expired, waiting for approval) are
  returned only after the door id matched. They are not terminal. The joiner's
  daemon stores the last refusal so status can say why, and waits minutes, not
  seconds, before trying a full or closed door again.

### Consent and the public page

Three ways to carry consent inside the join were considered and rejected.

- A consent signed by the joiner before admission can never be valid: an event
  counts only if its author was a member at the governance position it names,
  and that position does not exist until the administrator signs the admission.
- Dropping only the active-member clause moves the suspension to the
  newcomer's first event.
- Hiding a non-consenting author's work fails the snapshot validator, which
  requires task states to agree with the attempts and results shown.

The rule that works is small. The joiner's reviewed join stores the public
name, and the joiner's daemon authors the consent as that agent's first event
once admission is effective. In `eligible`, on a joinable farm, a member who
came through the door and has no valid consent (none yet, declined, outdated,
disputed) gets a fixed placeholder profile instead of failing the farm. The
snapshot schema is unchanged. A newcomer appears as "Participant" for a second
or two and then under their name; someone who withdraws consent goes back to
the placeholder. Nothing a door-admitted member does or omits can suspend the
page.

### Rules a joinable goal must have

A door may open only when, for the goal's defaults and every task type:

- only a closed selector (a role or a named participant) may open tasks;
- completion is safe: its deciding selector is closed, treating `all` as safe
  when one branch is and `any` when every branch is;
- every offered start is offered by a closed selector;
- the shared tree, if used, has an explicit safe completion rule;
- there are no stages.

The check is one pure function beside formation validation. It runs when the
door opens and whenever the administrator's daemon signs new rules, a task
revision or a workspace epoch while the door is open. Roles and named
participants may not include a door-admitted key.

The `public` preset, read against the validator but not run:

```json
{"schema_version":2,
 "roles":{"maintainer":{"description":"Opens tasks and approves results. One or more members."},
          "lead":{"description":"Picks the result to use and closes tasks. One member."}},
 "work":{"propose":{"kind":"role","name":"maintainer"},
         "publish":{"kind":"members"},
         "starts":[{"kind":"independent","by":{"kind":"members"}}]},
 "decisions":{"completion":{"kind":"reviews","by":{"kind":"role","name":"maintainer"},"count":1,"exclude_author":false},
              "selection":{"kind":"role","name":"lead"},
              "finish":{"kind":"role","name":"lead"}}}
```

Two roles are needed because a selection authority binds exactly one member. A
joiner's result is final only once the lead selects it: an author can withdraw
their own unselected events by signing a second event at the same position.
Tasks are opened by maintainers for the same reason, since a task opened by a
joiner could be withdrawn along with the work under it.

### The joiner's path

One owner command, in the plan-then-yes style of `up` and `goal add-local`:

```sh
locust --owner join <farm-address> --agent NAME --plan
locust --owner join <farm-address> --agent NAME --yes --review ID --name "Public name" --allow contribute
```

- The command fetches the descriptor once, verifies it offline and prints the
  plan: goal title, host key fingerprint, that the whole goal becomes readable,
  what is public and under which name, what the joiner's own agent would be
  allowed to do, seats, expiry and whether the host was seen recently.
- `--yes` is refused without the review id from `--plan`, and `--name` and
  `--allow` have no defaults.
- It records the join intent, the public name and the local grants in one
  transaction, then waits with an explained status.
- The join command must be able to prove the descriptor belongs to that farm
  even if the service is hostile. Today nothing signed by a farm's publishing
  key names a goal, so any administrator can claim any farm id. The publishing
  key should sign the goal id inside the publication record.

The farm page also offers a prompt to paste into a coding agent. It is a second
prompt with its own contract document and word-for-word test, like the setup
prompt in [first-contact.md](../docs/first-contact.md). It refers to the setup
steps without quoting the setup prompt (which forbids joining), tells the agent
to stay in the same chat, to show the plan unchanged, to take the public name
and permissions from the person, and to stop using `--owner` once the join is
done.

### The host's path

- `goal create` with the `public` formation, then `farm on --joinable`, whose
  plan folds in the host's own consent and grants the administrator agent what
  unattended admission needs.
- `farm door open --seats N --expires D`, `farm door close`, and in `ask` mode
  `farm door pending`, `admit` and `deny`.
- `farm door status`: one row per member with public name, how they came in, a
  key prefix, when they joined and when they last synced. `member remove`
  accepts a unique key prefix or public name. Today members print as 64-hex
  keys and a remote admission appears nowhere the host looks.

Hosting is a live commitment. People get in only while the host's daemon is
online, and their work counts only when the host's maintainer agents review it;
nothing wakes those agents. A door meant to stay open for days needs the
administrator's daemon on a machine that stays on.

### Joining a goal in progress

- The administrator's log is sent first, then the current rules definition and
  the accepted workspace head, before the rest of history. Today everything
  after the genesis payload arrives in hash order, so a newcomer can see no
  startable work for most of a long download.
- The first read is a brief: whether catch-up is complete, the formation's
  guidance and a host-chosen document (both marked as host-written text, not
  instructions), the tasks this agent may start with their titles, and its
  permissions in words.
- History before admission is browsable but not counted as unread.
- Under `members` starts a newcomer can attempt tasks opened before it joined.
  The code allows this; no test covers a member admitted after the task exists.

### Limits and guards

- **Ceiling.** Sixteen active members per joinable goal, host included, enforced
  in `plan_join`; eight if the qualification below is not run before release.
- **Two cheap fixes first.** Each completed exchange is a durable commit on
  both daemons, and running out of dial slots is counted as a failure of the
  peer being dialed. Both are small and both decide how far the ceiling can go.
- **Published endpoint.** Unadmitted inbound connections get a small receive
  window that is raised on admission, a fixed count of 64 to 128, address
  validation before a permit is taken, and 10 seconds instead of 30.
- **Fetching.** Payloads stay automatic. Artifacts and file trees named by a
  door-admitted author are fetched automatically only by daemons whose agent
  holds a deciding role, up to a quota; everyone fetches once the result is
  selected or integrated.
- **Addresses.** Every member's daemon can learn every other member's IP
  address. A relay-only setting is about ten lines over an existing transport
  mode and should be offered to hosts and joiners.

### Farm service

- The descriptor travels as a new optional field beside the snapshot, is stored
  as opaque text, and is served only from `/api/farms/<id>/join`. Seats and
  expiry go in the service view.
- Enrollment is part of this design, not a prerequisite. The recommended rule:
  anyone may publish a link-only farm and open a door on it; listing in the
  gallery stays for operator-enrolled farms. The gallery is what makes doors
  enumerable.
- The farm page should fall back to polling when its event stream is refused.
  The service caps streams at 512 in total, and a QR code exists to put many
  people on one page.

## What the review changed

| First design | Correction |
| --- | --- |
| A multi-use ticket with a secret, rotated by revoke and reissue | A public door id that never changes; rotation dropped. Retrying a refused join only works if the id is stable |
| Consent carried in the join request, three options open | None of the three works; placeholder rule in `eligible` plus consent as the joiner's first event |
| A member who declines is removed after a grace period | Removal does not restore the page; replaced by the placeholder rule |
| `independent-attempts` and `review-panel` are safe | The first lets any joiner complete any task; the second lets joiners open tasks. Only `public` qualifies |
| Rules checked on the current binding | Tasks and the shared tree keep the rules they were pinned to; check all three, or start from a fresh goal |
| One `maintainer` role | Two roles; a selection authority binds exactly one member |
| Opening an existing goal with everyone's consent | Refused; joinable from creation only |
| Seats as a per-ticket count | Seats consumed on the chain, plus a ceiling on active members |
| Approval of each joiner deferred | Same record carries both modes; `ask` is the only answer to seat exhaustion |
| Farm id check binds the ticket to the farm | It does not; the publishing key must sign the goal id |
| The farm address is about 45 characters | 57; a typed alias is the real handoff from a phone |
| Enrollment is a prerequisite | It is a design decision that sizes the service |

## Alternatives rejected

- **Guest contributions without membership.** A non-member can read nothing,
  and the public snapshot carries no task text, so proposals would be blind.
- **Admission hosted at locust.farm.** The admitter must hold the
  administrator's key and the administrator is fixed at genesis, so this is
  hosted goals: a different product with accounts and stored content.
- **A single-use ticket minted per scan.** Needs a channel from the service to
  the daemon that does not exist, and whoever fetches one holds it anyway.
- **An automatic bridge from a public goal to a private one.** Large, and it
  does not shrink what the host must review. The bridge stays manual.

## Risks to state, not engineer away

- A joiner reads the whole goal and keeps what it received after removal.
- Identities are free. Seats, pacing and the ceiling slow abuse; only `ask`
  mode puts a scarce resource, the host's attention, in the way.
- Reviewing or attempting a stranger's work runs their code on your machine.
  Locust does not sandbox an agent's tools. This applies in both directions: a
  joiner who allows `execute` runs the host's tasks.
- Findings, task text, file trees and the brief are untrusted text that other
  members' agents will read.
- Removal is forward-only and is not a ban.
- No document states a threat model for hostile members. One should be accepted
  in `docs/` before this is built.

## Decisions for the owner

1. Is a joiner a full member of the goal the page shows, with joinable goals
   public from creation? Recommended: yes.
2. May a door-admitted member's activity appear under an unnamed placeholder
   until their signed consent arrives, and again if they withdraw it?
   Recommended: yes. The gallery's sentence "Every member of a farm agreed to
   publish it" then needs rewording for joinable farms.
3. Which door mode comes first? Recommended: build the record with both, ship
   `open` first and `ask` before joinable farms are listed in the gallery.
4. May the joiner's agent run the join after the person says yes in chat?
   Recommended: yes, with the review id and no default name or permissions.
5. May the one-step join grant `execute`? The red team says no, per task only;
   the journey review says a joiner without it is sent back to the terminal for
   every task. Recommended: an explicit choice with no default.
6. May a joinable farm publish the host's public key, endpoint id, goal id and
   a relay address? This reverses a tested promise. Recommended: yes, stated in
   the host's plan.
7. Enrollment on locust.farm. Recommended: open for link-only farms, operator
   enrollment for listing.
8. Is the first version a live session, with an always-on machine advised for
   doors open longer? Recommended: yes.
9. Are seats consumed for good, with only the host able to add more?
   Recommended: yes.

## Build order

1. The two sync fixes and the pre-admission budget.
2. Door record, `plan_join` branches, the admission tag, refusals, seats and
   ceiling. Protocol and API versions go to 7 and local state is recreated.
3. The placeholder rule, consent as the joiner's first event, adapted farm
   tests.
4. The `public` preset, the safety check and the late-joiner tests.
5. Descriptor field, service route and view, page door state and QR code.
6. `locust join`, the join prompt and its contract.
7. `ask` mode and the host's roster.
8. Priority fetch, the brief and the unread baseline.
9. Qualification and a release from current source.

Steps 2 to 6 give a working open door on one machine.

## Qualification before release

- A join between two Macs on two networks through the relay, including the
  host's laptop sleeping and waking. No two-machine run is recorded today.
- Counters in `doctor --json` for exchanges, commits, refolds and refused
  dials, then simulator scenarios at 8, 16, 24 and 32 members in
  [scripts/simulate_machines](../scripts/simulate_machines): sequential joins, a
  burst, half and three quarters of members stopped, a connection flood against
  the host during a join, and a full door with waiting joiners. The ceiling is
  the largest size that passes.
- One farm page with a few hundred viewers.

## Documents this reverses

Recording this as an accepted decision would change these in the same step:

- "Showing a farm and joining its private goal are separate operations"
  ([architecture](../docs/swarm-visualization-plan.md)).
- "Joining a goal is not consent" and that keys and addresses are never
  published ([farm guide](../docs/guide/farm-publication.md)).
- "Only the first agent to use it can join"
  ([collaboration guide](../docs/guide/collaboration.md)), true for private
  tickets only.
- "Every member of a farm agreed to publish it" and "you decide who joins" on
  the website.
- "Approvals that stay separate" in [first-contact.md](../docs/first-contact.md).
