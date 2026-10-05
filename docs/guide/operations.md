# Run the daemon

The daemon is the locust.farm process that stores goal data and talks to other
members' daemons. `locust up` runs it as a service.

## Start the daemon

```sh
locust --home /PATH/TO/HOME daemon run
```

It runs in the foreground and creates the data directory and owner credential on
first start. Stop it with Ctrl-C or
`locust --home /PATH/TO/HOME --owner daemon stop`.

## Run it as a service

Every `service` command takes the same five flags:

```sh
locust service plan --prefix ~/.local/share/locust --kind launchd \
  --profile-home ~ --daemon-home ~/.locust --log-dir ~/.locust/logs
```

`service apply`, `start`, `status`, `stop`, `remove-plan` and `remove` take them
too. `apply` and `remove` also need `--expect-plan PLAN_SHA256` from the plan.
Remove only a stopped service; data and logs stay.

`--kind launchd` is for macOS; `--kind systemd` (a user unit) is untested on
Linux. Start and stop can take a moment; check `service status`, then
`locust --owner doctor`. After an upgrade, `service start` loads the new code.
locust.farm refuses to change a unit it did not create.

## The data directory

The data directory is `--home`, else `LOCUST_HOME`, else `~/.locust`. It has mode
0700. Use the same `--home` for every command.

It holds the socket `daemon.sock`, the lock `daemon.lock`, the database
`locust.db` and stored files in `blobs/`. Credentials live in `owner.credential`,
`agents/` and `authors/`. `onboarding/` holds setup progress and
`context-receipts/` read receipts; `sessions/` and `logs/` appear when used.

locust.farm does not read data from older versions. Start with a new data directory.
Uninstalling locust.farm never deletes it.

## Environment variables

- `LOCUST_HOME`: the data directory (absolute path).
- `LOCUST_CREDENTIAL`: the credential file; no default.
- `LOCUST_SESSION`: the session secret file.
- `LOCUST_RELAY`: `n0` (default), `none` or a relay URL.
- `LOCUST_LOOKUP`: `all` (default), `local`, `mainline` or `none`.
- `LOCUST_BIND`: `IP:PORT` to listen on; default all interfaces.

See [What leaves your computer](sharing.md#what-leaves-your-computer).

## Offline work and restarts

Members keep working offline when their daemon holds the records they need.
Membership changes, rule changes and new stage tasks wait for the administrator.
A decision waits for its decider. After a restart, the daemon resumes unfinished
deliveries.

A timeout does not mean a request failed. Check the state, then retry with the
same values.

## Conflicts

- Draft edits name the revision you expect (`--expected-revision`). If the draft
  changed, locust.farm refuses the edit.
- Two conflicting decisions stop decisions for that task. Both records are kept;
  other work continues. The administrator can start a new round with
  `task revise`.
- `patch apply` refuses a dirty checkout or a changed base.

## Cancelling work

The worker, or the member who offered the work, can request a stop with
`attempt cancel --goal GOAL --attempt ATTEMPT`. The request stays open until the
worker runs `cancel acknowledge` with `--outcome stopped`, `completed` or
`uncertain`. Revoking a permission does not stop a running process.

## Backups

There is no backup or restore command. Stop the daemon before you copy the data
directory; it locks the database while it runs. Restoring a copy is untested.
