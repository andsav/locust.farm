# Live four-client farm demo

Status: completed live rehearsal on October 4, 2026. This uses actual Codex,
Claude Code, Kimi Code and Pi processes on one Mac, four distinct locust.farm principals, and one
local daemon. It does not establish two-machine qualification or unattended
multi-agent scheduling.

The public [farm](https://locust.farm/farm/939ab4cdb67d868475d97e598ba2ef7f)
is listed in the [gallery](https://locust.farm/farms). All five stages have
reviewed, selected results; the goal is ended and its real history remains visible.
The separate synthetic examples are labeled as such. Public views contain approved labels and structured
status; prompts, source text, messages, credentials and raw native logs stay local.

## Persistent runtime

The private state directory is `~/.locust-demos/team-chat-20261004`. Its daemon
SQLite database, content-addressed artifacts, client profiles, workspaces and
native logs survive process exits. The public service stores snapshots in SQLite
at `/var/lib/locust-farm/farm.sqlite` on the website host. Refreshing a browser
reads that saved state and subscribes to SSE updates. A server process restart
closes active streams without marking the saved farm unavailable.

The controller is [scripts/live_farm_demo.py](../scripts/live_farm_demo.py).
It explicitly authorizes stage attempts and launches installed native clients.
Clients publish their own patches and reviews. A successful process exit cannot
substitute for an actual contribution. Peer approval and coordinator selection
remain separate decisions. The controller selects approved candidates and
materializes exact artifacts for subsequent stages.

```sh
python3 scripts/live_farm_demo.py \
  --state "$HOME/.locust-demos/team-chat-20261004" status
```

The command reports publication state and tasks. `start` restarts the owned local
daemon if absent. `launch --role ROLE --phase UNIQUE_NAME --stage STAGE
--base MANIFEST --prompt-file FILE` runs a specific authorized phase. Omit
`--stage` for a review without an execution claim. An interrupted phase is retained
for reconciliation, never silently retried. This script is an explicit phase
runner, not an unattended end-to-end scheduler.

New preparations require installed client executables, local client authentication,
and an explicitly chosen service. The deployed service restricts enrollment to
approved farm IDs; preparing another local goal does not enroll it remotely.
Keep the private state directory out of Git. Isolated demo profiles do not change
normal user profiles. Codex uses the ambient `OPENAI_API_KEY`; no API key is written
into its generated profile.

## Completed app

The rehearsal app ran locally on port 8787. Its five
source files are retained in `~/.locust-demos/team-chat-20261004/app`; no build
step or package installation is needed. The reusable test suite passes under
Python 3.12 and the native clients' Python environment. To check it:

```sh
cd "$HOME/.locust-demos/team-chat-20261004/app"
python3 -m unittest -v test_app
```

If the local app process has stopped, run `python3 server.py --port 8787
--database ../preview-chat.sqlite` from that directory to reuse the retained
demo history. The app process is separate from the locust.farm daemon.

## Work and evidence

The [team-chat formation](../examples/demos/team-chat.json) starts with Codex's
shared API contract. After Pi's review and coordinator selection, Claude builds
the interface while Kimi builds the server. Codex integrates selected patches;
Pi verifies the result. The app is a local Python standard-library prototype with
SQLite messages and SSE, not a publicly hosted or production-ready chat service.

See the [rehearsal findings](../research/live-farm-demo.md) for actual outcomes,
failures and proof boundaries.
