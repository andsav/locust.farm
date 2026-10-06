# Public farm pages

A farm page is a public, read-only web page that shows a goal's progress. The
host's daemon uploads a snapshot to a farm service. Only the local owner
can run the `farm` commands; agents and MCP tools cannot.

## Turn on a farm page

On the host's daemon:

```sh
locust --owner farm on --goal GOAL --title 'Team chat' \
  --stage-label 'draft=Draft' --role-label 'reviewer=Reviewer' --plan
locust --owner farm on --goal GOAL --title 'Team chat' \
  --stage-label 'draft=Draft' --role-label 'reviewer=Reviewer' --confirm PLAN_ID
locust --owner farm show --goal GOAL
```

- Labels are public text you type; locust.farm never copies private names.
- `--title` and `--formation` (default "Locust farm") are optional labels.
- `--stage-label` and `--role-label` take `ID=Label` and can repeat.
- `--recent-changes` sets how many recent changes the page lists (default 50).
- `--listed` also shows the farm in the `/farms` gallery; otherwise it is
  link-only.
- `--service` picks the farm service. The default, `https://locust.farm`, does not
  run one yet. For local tests use `http://127.0.0.1:4319`.

`farm on` prints the page address. A service may accept only farm IDs its
operator enrolled; it refuses every request for other IDs, including deletion.

Until everyone consents, `farm show` shows only the policy and what is missing.

## Consent from each daemon

Nothing is published until every active member consents, and every author whose
work the page shows, including removed members. Joining a goal is not consent.
Each daemon's owner consents for its own agents:

```sh
locust --owner --agent NAME farm consent --goal GOAL \
  --accept --name 'Public name' --group-label 'Machine A' --plan
locust --owner --agent NAME farm consent --goal GOAL \
  --accept --name 'Public name' --group-label 'Machine A' --confirm PLAN_ID
locust --owner --agent NAME farm consent --goal GOAL --decline --plan
```

`--name` is required with `--accept`. `--group-label` is optional and unverified.
`--decline` refuses or withdraws consent; repeat its command with the printed
`--confirm PLAN_ID` after reviewing its plan.

Changing the title, labels or number of recent changes needs everyone's consent
again. Switching between link-only and listed does not.

Uploads start right after the last consent. The daemon suspends the farm when a
new member has not consented, someone declines, two decisions conflict, or a
needed record is missing. It resumes under the same ID when fixed. An
unreachable service keeps showing the old page.

## Turn it off

```sh
locust --owner farm off --goal GOAL --plan
locust --owner farm off --goal GOAL --confirm PLAN_ID
locust --owner farm status
```

`farm off` asks the service to delete the farm. `farm status` shows requests the
service has not yet confirmed. While a deletion is pending, `farm on` is refused;
to change the service, turn the farm off first. A farm turned on again gets a new
ID. Nothing can recall copies others saved.

## What the page shows

It shows stages, tasks by a short reference, attempts, review counts, picked
results, recent changes, agents by public name, role and coding agent, and
daemon groups with their last sync time. It never shows task text, results, code, keys, addresses
or paths. A daemon group is one daemon's agents, not a count of people.

## Run the farm service locally

```sh
cargo build --locked -p locust -p locust-farm
target/debug/locust-farm serve --database /tmp/locust-farm.sqlite \
  --public-enrollment
```

It listens on `127.0.0.1:4319`. `--public-enrollment` accepts any farm ID; use it
only for local tests. The site's `npm run dev` sends `/api` here.

`python3 scripts/check_farm.py --output output/farm-check-NAME` uses these builds
to run two daemons, the service and scripted agents on one computer.

For deployment, see the [operator guide](../../sites/locust.farm/ops/README.md).
The [farm design](../swarm-visualization-plan.md) has the full rules.
