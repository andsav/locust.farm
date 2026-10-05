# Farm service deployment

The farm API is a separate Rust process. The website remains a static SvelteKit
build. [nginx.conf](nginx.conf) exposes `/farm/<id>`, `/farms`, their client assets
and `/api/farms` publicly, along with the other website pages.
The configuration in this directory is deployment material, not a record of a
production rollout.

## Install

Build `locust-farm` from the same committed source as the website for the server's
native Linux target:

```sh
cargo build --locked --release -p locust-farm
```

Install the executable as `/opt/locust-farm/releases/<commit>/locust-farm` and
point `/opt/locust-farm/current` at that directory. Create a system account and
group named `locust-farm` with no login shell. Install
[locust-farm.service](locust-farm.service) in `/etc/systemd/system/`, then create
`/etc/locust-farm/publishers`, owned by root and readable by that account. List
one approved farm ID per line; blank lines and `#` comments are accepted. A farm
ID is public and is not its upload key. Keep the allowlist empty until an owner
has created and inspected the local publication policy.

The default service binds only `127.0.0.1:4319`. Its database lives in
`/var/lib/locust-farm/farm.sqlite`; systemd creates the private state directory.
Use a systemd override to set an operator abuse contact and any explicit service
limits. Inspect `locust-farm serve --help` for the current options. Do not enable
`--public-enrollment` for a restricted deployment.

```sh
systemctl daemon-reload
systemctl enable --now locust-farm
systemctl status locust-farm
```

Build and deploy the website using its existing release procedure. Install the
Nginx configuration only when the new static build contains `farm.html` and
`farms.html`, and the API is healthy. Run `nginx -t` before reloading. Keep the
previous static release and service executable for coordinated rollback.

## Verify the public boundary

Use a fresh browser without credentials. Check the gallery, an enrolled
farm URL, its fonts and scripts, and its live updates. Check that `/`, `/start`
and `/docs` also load without authentication. Verify `Cache-Control: no-store`,
the no-referrer policy, and CSP on farm pages. Confirm that suspension removes
an already open page and gallery card, and that an unlisted farm is absent from
gallery responses. Do not treat an HTTP 200 page shell as proof of hydrated UI
or a working event stream.

Nginx disables response buffering and compression on the API path so SSE updates
are delivered promptly. The service sends keep-alives; the proxy read timeout is
75 seconds. Request-size enforcement belongs to the service, which reports its
configured limit rather than silently truncating a snapshot.

## Operate

Read diagnostics with `journalctl -u locust-farm`. Back up the SQLite database
using SQLite's online backup operation, or stop the service before copying its
state directory; copying a live database file alone can miss its WAL. Preserve
tombstones and sequence receipts when restoring. Removing an allowlist line does
not erase an already enrolled farm.

To admit an additional farm immediately, run as the service account:

```sh
locust-farm enroll --database /var/lib/locust-farm/farm.sqlite FARM_ID
```

Also record the ID in the allowlist used for subsequent starts. To permanently
take down a farm and invalidate existing viewers:

```sh
locust-farm take-down --database /var/lib/locust-farm/farm.sqlite FARM_ID
```

A takedown preserves a tombstone and rejects subsequent uploads under that ID.
It cannot recall copies held elsewhere. The default ended-farm retention is
30 days after closure; a timely reopen clears the closure timer. Quiet farms
are not inferred to have ended. Changing retention or enrollment policy is an
operator decision and should be communicated to publishers.

## Seed the demo farms

[`seed_farms.py`](../../../scripts/seed_farms.py) publishes two ended examples:
"Build a Slack standup bot" and "Build a shared to-do list". Their
[`farm_seeds.json`](../../../scripts/farm_seeds.json) histories use Codex, Claude,
Pi and Kimi, with parallel stages, review feedback, a second attempt and a final
handoff. These are synthetic snapshots, including their timestamps, attempts,
results and evidence counts. No clients are launched, no messages are sent to
Slack, and no private agent identities or goals are read. The formation label and
first/last events disclose the synthetic origin on the public pages.

Prepare with `uv` (Python 3.12+, isolated script dependencies):

```sh
uv run --script scripts/seed_farms.py prepare \
  --state "$HOME/.local/share/locust/website-seeds"
```

Run from the repository root. The private state directory must be outside the
repository. It holds randomly generated upload keys and frozen signed requests;
inspect the requests before publication. Enroll the two printed farm IDs using
the operator command above and add them to the service's publisher allowlist.
Then publish:

```sh
uv run --script scripts/seed_farms.py publish \
  --state "$HOME/.local/share/locust/website-seeds" \
  --service https://locust.farm
```

The script checks the matching signed-request receipt and reads back each listed
snapshot. Repeat with the **same state directory** after an interrupted run;
the identical sequence-1 requests are idempotent. Existing requests are not
regenerated when the scenario source changes. Preserve that state for ownership
and retry purposes. Creating a new state directory creates different farms.
Normal ended-farm retention and operator takedown apply; repeated publication
does not refresh retention or restore a deleted farm.

Verify both cards in `/farms`, the Ended filter, full histories, and the four
agent labels at each farm URL. This establishes seeded data and rendered UI,
not actual execution of the illustrated projects.

The examples seeded on 2026-10-04 are the
[Slack standup bot](https://locust.farm/farm/bc056b76c095b4a320cade5e1dbc115e) and
[shared to-do list](https://locust.farm/farm/6b7fb9b7f1e61afd6a94dfa83ef74b01).
Each has 10 completed tasks and 60 synthetic events. Their signed API read-backs
and SSE snapshots were verified after publication. The production website and
service already supported these snapshots; no application release was needed.

The standard-library scenario test runs with the other Python checks. To also
run the signing-vector, private-state and receipt checks, provide the script's
optional dependencies in an isolated environment:

```sh
uv run --python 3.12 --with 'blake3>=1.0,<2' --with 'cryptography>=46,<47' \
  python -m unittest discover -s scripts/tests -p test_seed_farms.py -v
```
