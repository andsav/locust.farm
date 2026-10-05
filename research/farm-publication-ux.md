# Farm publication usability audit

Status: audit of 2026-10-04 at `3a6baed`. It records verified results and
proposals. Nothing proposed here is implemented.

The question was whether it is clear, from every surface a person touches, how to
make a farm public or not: for people who ask their coding agent, and for people
who want visible controls and numbered steps.

## Method

Five audits, each then checked finding by finding by a separate reviewer:

- the website source, read statically;
- the running website, clicked through at desktop and phone widths against a
  local farm service with two seeded farms, then compared with production using
  GET requests only;
- the terminal, following the [farm guide](../docs/guide/farm-publication.md)
  literally with two isolated daemons and a local farm service;
- the agent path: the [installed skill](../skills/locust/SKILL.md), the MCP
  tools, `llms.txt` and the setup prompt;
- the manual, read as a procedure.

Six journeys were judged: share by link, list in the gallery, switch between
the two, take down, tell the current state, and consent as a member.

Limits: one Mac, scripted commands, no real coding agent asked to publish. The
points marked "from code" were read in the source and not run.

## Answer

No. There is no click path, and the prompt path was not designed either.

- Publication is controlled only by `locust --owner farm on|off|show|status|consent`.
  The website has no control and no link from its farm pages to the instructions.
- The installed skill, the MCP tools and `llms.txt` never mention farm pages,
  consent or visibility.
- Two facts stop every journey before clarity matters: the published 0.1.0
  build has no `farm` command, and the hosted service accepts only farm IDs its
  operator enrolled.

## Verified results

### The journey is blocked today

- The published build is `0.1.0-cd65921d8a0f`
  ([release record](../docs/public-preview-release.md)). `farm` was added in
  `b916864`, which is not an ancestor of `cd65921`. The
  [overview](../docs/guide/overview.md) lists public farm pages under "What
  works today", and no page says a newer build is needed.
- The guide says the default service, `https://locust.farm`, "does not run one
  yet". Production answers `GET /api/farms` with listed farms. The
  [deployed unit](../sites/locust.farm/ops/locust-farm.service) runs with
  `--allowlist`, and the [rehearsal record](../docs/live-farm-demo.md) says the
  service restricts enrollment to approved IDs. No user-facing page says how to
  get an ID enrolled.
- A refused first publication locks the goal. Run against a local service
  without `--public-enrollment`: `farm on` reports only
  `"last_error": "farm service returned HTTP 403"`. `farm off` then leaves
  `"pending": "delete"` forever, because the service also refuses the deletion.
  After that, `farm on` with any service fails with
  `conflict: farm deletion is pending; wait for its receipt`. The checks are in
  [the daemon's farm requests](../crates/locust-core/src/node/farm.rs) and
  [the service](../crates/locust-farm/src/lib.rs).

### The website

- `/farms` says "Swarms whose creators chose to list them publicly. Farms shared
  only by link are not shown here." It has no link to how. Its empty state reads
  "No farms are listed yet." and its error state offers only "Try again".
- A farm page's only links for an owner are "Browse farms" and "Copy link". Its
  "Visibility" text is the same for listed and link-only farms, although the
  payload carries `visibility`.
- "Farm unavailable. It may have been unpublished, or the link may be wrong."
  is shown for a farm waiting for consent, a suspended farm, a deleted farm and a
  wrong link alike. The address `farm on` prints shows this page until the last
  member consents.
- The only instructions are the manual page "Public farm pages": two clicks from
  the home page, the tenth of fourteen cards under Docs. "Sharing and privacy",
  the concepts page and the glossary do not mention farm pages. Searching the
  manual for "unpublish" finds nothing.
- The Formations editor already shows a pattern that fits this static site:
  build with controls, then copy a command or a prompt.

### The terminal

- Every farm command prints one heading and raw JSON. No output names a state
  or a next step. After the administrator's own `farm on`, the reason shown is
  `"publication policy has not arrived"`.
- `farm consent --decline` prints "Local publication consent recorded for the
  current policy."
- A member is not told that publication was requested. `status`, `inbox`,
  `goal status` and `pending` are silent, and the member's `farm status` prints
  `"farms": []`. The administrator is not told when a decline takes the page
  down.
- Switching visibility has no command of its own. `farm on` replaces the whole
  policy with the flags of that call. Re-running it with only `--listed` set the
  title to `null`, emptied both label maps, reported
  `"reason": "publication policy has changed"` and took the page offline
  (`GET /api/farms/<id>` returned 404). Re-running with the original flags
  restored it without new consent. The guide says only that switching needs no
  new consent. No daemon or pipeline test switches a published farm between
  the two.
- Nobody can preview the page: `farm show` has `"snapshot": null` until every
  consent is in, and the last consent starts the upload.
- Stage and role IDs for `--stage-label` and `--role-label` appear in no goal
  output. They are found only in the formation source.
- These already work: the refusals for a missing `--owner` and for an agent
  credential, the message for `--accept` without `--name`, `--goal` taking the
  goal's title, the invitation text saying that joining is not consent, and the
  service keeping link-only farms out of the gallery.

### The agent path

- An agent credential cannot call the farm operations, the MCP server has no
  farm tool, and the skill's `locust-cli` wrapper refuses `--owner`. Nothing the
  agent reads tells it to hand the commands to its person.
- The refusal it meets, "farm controls require --owner without --as,
  --credential or --session", points it at `--owner`. From code: an agent with a
  shell and the same user account can run `locust --owner farm on`, and
  `farm consent --accept --name ...` with a name it chose. The owner-only rule
  holds against the agent credential, not against a shell.
- An agent can read whether a farm is live and listed from the public
  `GET /api/farms/<id>`, but no document says so.

### Expectations the pages set wrongly (from code)

- Link-only is not private. The address stays the same; only `farm off` ends
  public access.
- Only the administrator's daemon uploads, suspends and deletes. A member's
  decline takes the page down when that daemon has synced the decline and
  reached the service, not before.
- The service deletes an ended farm 30 days after the goal closed. The guide and
  the farm page do not say so.

## Proposals

In order.

1. Make the guide true. Say which build has `farm`, whether publishing on
   locust.farm needs enrollment and how to ask, and what HTTP 403 means. Let a
   farm that the service never accepted be dropped locally.
2. Remove the switching trap. Either `farm on` keeps the stored policy for flags
   that are left out, or a separate command changes only the visibility. Add a
   test.
3. Print a state line and a next step before the JSON on every farm command,
   for example "Not public yet: waiting for consent from bob (another daemon).
   Send them: ...". Say "declined" after a decline.
4. Tell the people involved. Show a pending item on each member's daemon when
   publication is requested, and tell the administrator when the page goes
   offline.
5. Give the website a path. Link "Publish your own farm" from `/farms`, its
   empty state and each farm page. Explain the causes on "Farm unavailable".
   Show "Listed in the gallery" or "Link-only" on the farm page.
6. Rewrite the guide as numbered tasks: share by link, list, unlist, take down,
   consent as a member, check the state. Give accept and decline separate code
   blocks, and add the message an administrator sends to members.
7. Add a builder page in the style of the Formations editor. It asks for the
   action, the title and the labels, then outputs the administrator's command,
   one consent command per member and a prompt for an agent.
8. Add a short farm section to the skill and an entry in `llms.txt`: what the
   agent may do, the exact commands to hand over, and how to read the public
   state.
9. Use one set of words everywhere: farm page, link-only, listed, turn off.

## Open decisions

- Whether locust.farm stays allowlist-only, and how a person asks to be
  enrolled.
- Whether an agent with a shell may run the farm commands when its person asks,
  as the setup prompt already allows for other owner commands, or whether a
  person must type them.
- Whether the manual stays public. It holds the only publishing instructions,
  while farm pages are public.

## Reproduce

Build `locust` and `locust-farm`. Start two daemons with temporary homes,
loopback binds and no relays, as [the farm check](../scripts/check_farm.py)
does. Run `locust-farm serve --bind 127.0.0.1:PORT --database FILE
--public-enrollment`, and a second service without `--public-enrollment` for the
refusal case. Create a goal on the first daemon, invite the second, then follow
the farm guide, always passing `--service http://127.0.0.1:PORT`. For the
website, point the site's `/api` proxy at a local service and publish the two
demo farms with [the seed script](../scripts/seed_farms.py).
