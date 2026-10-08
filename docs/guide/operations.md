# Run the daemon

The daemon is the locust.farm process that stores goal data and talks to other
members' daemons. `locust --owner up` runs it as a service.

## Start the daemon

```sh
locust --home /PATH/TO/HOME daemon run
```

It runs in the foreground and creates the data directory and your personal credential on
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

Beside the data directory, never inside it, Locust keeps the marks directory:
the home's path with `.marks` added, `~/.locust.marks` for the default home. It
holds the last record this computer signed in each goal, so Locust can tell
when the data directory was put back from an older copy. `locust --owner
doctor` checks it.

Uninstalling locust.farm never deletes the data directory.

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
Membership changes, rule changes and new stage tasks wait for the host.
A pick or close waits for the member named by its rule. After a restart, the daemon resumes unfinished
deliveries.

A timeout does not mean a request failed. Check the state, then retry with the
same values.

## Conflicts

- Draft edits name the revision you expect (`--expected-revision`). If the draft
  changed, locust.farm refuses the edit.
- Two conflicting decisions stop decisions for that task. Both records are kept;
  other work continues. The host can start a new round with
  `task revise`.
- `workspace integrate` pins the expected accepted head and workspace epoch.
  Conflicting decisions dispute that workspace boundary; arrival order does not
  choose a file tree.
- `workspace update` preserves compatible local edits and refuses conflicts or
  changed file identities. An interrupted update uses `workspace recover` with
  the exact operation ID; unknown filesystem states require inspection.

## Cancelling work

The worker, or the member who offered the work, can request a stop with
`attempt cancel --goal GOAL --attempt ATTEMPT`. The request stays open until the
worker runs `cancel acknowledge` with `--outcome stopped`, `completed` or
`uncertain`. Lowering a level or revoking a task allowance does not stop a running process.

## Backups

There is no backup or restore command. Stop the daemon before you copy the data
directory; it locks the database while it runs. Copy the data directory and not
the marks directory beside it.

A copy of the data directory holds the keys of every goal this computer hosts
and of every agent on it. Whoever starts a daemon on a copy can act as you in
those goals, so keep copies private. Never run two copies of the same data at
once; nothing can protect a goal from that.

A copy you put back is older than what this computer signed. Locust notices,
signs nothing in the goals the copy is behind in, and catches up from the other
members' computers by itself. `locust --owner status` lists each goal that is
catching up, what it waits for and the command that continues it. Pending
invitations of the goals you host are revoked, because a copy cannot know which
were used. Levels, allowed tasks, connected folders, and which agents are
disconnected or have left a goal are as they were in the copy.

After a whole-computer restore or a move, the marks directory is a copy too, and
Locust cannot tell how old the data is. A goal you host then waits for you:
check that this is the newest copy of this computer's data and that no other
copy is running, then run `locust --owner goal continue --goal GOAL`, or
`--all`. It shows what it would do and asks you first. Continuing too early can
leave the goal unable to admit, remove or change rules for anyone. A goal
hosted on another computer waits to hear from that goal's other computers and
needs nothing from you.

A copy older than a goal you host loses that goal; nothing brings it back. A
goal you joined after the copy was made needs its ticket again.

## If the daemon will not start

When the daemon cannot open its data, it stops and prints what happened and
then what to do. It never deletes a damaged data directory, or one made by
another version, and never starts over on its own. Beside the data directory is
its marks directory, the same name with `.marks` added (`~/.locust.marks` by
default); the daemon needs both. To copy a data directory, see
[Backups](#backups).

| The message says | Do this |
| --- | --- |
| A daemon is already running on the data directory | Stop it, then start again. |
| Another program has the database open | Close that program (an `sqlite3` shell, for example), then start again. |
| The database has another schema or event protocol version | Start the Locust version that made the data directory, or move the directory aside, do not delete it, and start with a new one. No version converts it. |
| Stored data is corrupted | Move the data directory aside and do not delete it: it holds your keys and every record. |
| Storage failed | Check that the disk has space and that you can read and write the data directory and the marks directory, then start again. |
| The marks directory or its file has the wrong mode | Run the `chmod` the message names, then start again. |

A new data directory starts with no goals. Goals you host cannot continue from
it, and each goal you joined needs its ticket again.
