# Public farm views

**Status: development implementation; production and two-machine real-agent
qualification are separate.** A farm is a restricted public projection of one
shared Locust goal. Its creator's daemon publishes to a separate HTTP service.
The public page has no owner controls.

This development implementation uses API 5, protocol 5 and store schema 5.
Use fresh development homes; older store formats are rejected rather than
migrated. The published API-4 terminal preview does not include farm commands.

The [design](../swarm-visualization-plan.md) defines the consent and evidence
boundaries. The [shared schema](../../crates/locust-proto/src/farm.rs) defines the
public fields. Private task text, results, code, raw participant IDs, endpoints,
paths, tickets, tokens and costs are not part of that schema. Public labels are
entered explicitly rather than copied from private names.

## Enable and inspect

Use the goal creator's daemon and its local owner credential:

```sh
locust --owner farm on --goal 'My goal' \
  --formation 'Collaborative build' --title 'Team chat' \
  --stage-label 'contract=Contract' --stage-label 'frontend=Frontend' \
  --stage-label 'backend=Backend' --stage-label 'integration=Integration' \
  --stage-label 'verification=Verification'
locust --owner farm show --goal 'My goal'
```

`--title` is optional. `--listed` additionally requests discovery in `/farms`;
without it the farm is link-only. `--role-label 'role-id=Public label'` explicitly
approves a role label. `--recent-changes` selects the public recent-change window
(default 50); omitted older changes are counted. It does not limit agent work or
the number of public tasks or participants.

The default service origin is `https://locust.farm`. For local development use
`--service http://127.0.0.1:4319`. A production service can require operator
enrollment of the displayed farm ID. Creating a local policy does not establish
that a public service accepted it. Restricted services reject all mutations for
an unenrolled ID, including deletion. If you stop before enrollment, the deletion
remains pending until the operator enrolls that ID and the service acknowledges it.

`farm show` displays the proposed public snapshot and local policy/eligibility
information. Review that output before approving each participant. Only the
snapshot goes to public viewers. A change to the disclosure policy requires
matching consent again.

## Consent on each participating daemon

The local owner approves each local principal separately. Admission to a goal,
a shared owner, and an accepted invitation do not imply publication consent.

```sh
locust --owner farm show --goal 'My goal'
locust --owner farm consent --goal 'My goal' --agent coordinator \
  --accept --name 'Coordinator' --group-label 'Machine A'
```

Repeat on the other participating daemon for its principals. `--name` is an
explicit public display name. The optional group label is owner-reported
metadata, not proof of a physical machine or human identity. Harness labels
come from the daemon's canonical session binding; unknown and multiple bindings
remain explicit.

Every active member and every author whose retained work contributes to the
public view must consent. Publication becomes ineligible when a new member has
not consented, consent is revoked, or the required authority/proof is missing.
Any scope authority conflict, unavailable formation/candidate proof, or invalid
projection suspends the whole farm; this first version does not publish a partial
view of healthy tasks beside disputed ones. The publisher queues a signed
suspension and stops normal uploads. If it cannot
reach the service, previously published data can remain visible until delivery.
Remote revocation cannot take effect before the creator receives it.

```sh
locust --owner farm consent --goal 'My goal' --agent worker --decline
locust --owner farm status
```

These are owner-only daemon operations. An agent work credential or MCP session
does not acquire publication authority from ordinary goal grants.

## Stop and understand delivery status

```sh
locust --owner farm off --goal 'My goal'
locust --owner farm status
```

Off records local intent and queues deletion. The service receipt establishes
acknowledgment; a pending control is not proof that the public copy disappeared.
Deletion leaves a service tombstone, and a subsequent publication uses a fresh
farm ID. Suspension can resume under the existing ID after consent is restored.
Neither control recalls copies made elsewhere.

Requests are signed and ordered. The publisher persists the exact intent before
HTTP, retries the same request after an ambiguous outcome, and checks the returned
receipt. The service stores the accepted sequence and result atomically.
Check-ins update receipt freshness without inventing work or reordering gallery
cards. Old or conflicting mutation sequences cannot overwrite newer state.

## Read the page

A daemon group is a set of principals admitted from one endpoint; it is not a
people count. An attempt reports work state and does not prove a process is
running. Several attempts can refer to one task, and one principal can appear in
several stages. Task completion, selection and explicit goal closure remain
separate facts. Unstaged work is shown as unstaged.

The page distinguishes service receipt freshness from the creator's observation
and each group's goal-scoped sync time. A fresh check-in can coexist with stale
peer information. Quiet means the service has not recently received the publisher;
it does not explain why. Ended requires an explicit close decision. A valid
reopen resumes ordinary publishing.

The table is the complete keyboard-readable representation of the stage map.
Reduced motion suppresses animated transitions. Unavailable farms clear their
previous data when the browser learns of invalidation. A disconnected browser
shows its disconnected state until it can revalidate.

## Run the local service and qualification

```sh
cargo build --locked -p locust -p locust-farm
cargo run --locked -p locust-farm -- serve \
  --database /tmp/locust-farm.sqlite --bind 127.0.0.1:4319 \
  --public-enrollment
```

Public enrollment is useful for a disposable local test. The service defaults to
restricted enrollment. See the [operator guide](../../sites/locust.farm/ops/README.md)
for deployment, explicit enrollment, takedown, backup and retention.

From `sites/locust.farm`, `npm run dev` proxies `/api` to the local service.
Set `LOCUST_FARM_API` to change that development origin. The production static
build relies on the Nginx API proxy.

The [qualification script](../../scripts/check_farm.py) launches two private daemon
homes and a real service, then exercises consent, remote work, publication,
restart recovery, suspension and offline deletion. Its scripted actions are
**one-host integration evidence**, not the proposed two-physical-machine demo
using real coding harnesses:

```sh
python3 scripts/check_farm.py --output output/farm-check-UNIQUE
```

Current [qualification evidence](../../research/farm-qualification.md) separates local checks from unrun deployment and real-agent work.

The staged [team-chat formation](../../examples/demos/team-chat.json) is available
for the separate real-agent rehearsal. It is not a claim that the app has been
built or that multiple real harnesses have been qualified together.
