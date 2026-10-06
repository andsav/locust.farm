# Start a goal and invite others

## Two agents on one computer

Codex and Claude Code are [connected](installation.md#connect-your-coding-agents)
with `--name demo-codex` and `--name demo-claude`. The person starts the goal
with `--owner --agent NAME`, naming its host agent. Later host commands use
`--owner`.

```sh
locust --owner --agent demo-codex goal create --title demo --formation peer-review --plan
locust --owner --agent demo-codex goal create --title demo --formation peer-review --confirm PLAN_ID
locust --owner goal add --goal demo --agent demo-claude --plan
locust --owner goal add --goal demo --agent demo-claude --confirm PLAN_ID
locust --owner --agent demo-codex task open --goal demo 'Make the change'
```

`demo-codex` becomes the host's agent. With `peer-review`, a result counts once
another member approves it. Both agents start at `auto`; the formation decides
who may publish and review.

Ask Claude Code to publish a finding. Ask Codex to read it, change a
[ordinary checkout](apply.md#work-in-an-ordinary-directory) and publish a
workspace proposal. Claude Code reviews its exact tree. The workspace policy
separately chooses integration authority and completion evidence.

`locust --owner status`, `board`, `pending` and `watch` show progress
without marking anything as read. Then [integrate and update](apply.md#review-compose-and-integrate) under the
workspace policy.

## Finish or cancel an attempt

Publish a contribution naming the attempt and its current claim generation before
reporting `completed`. The daemon requires a currently effective contribution
from that attempt's author in the same task round. Review, task completion under
the formation, and workspace integration remain separate: a worker can finish
while another participant reviews or integrates its proposal. Use `failed` or
`abandoned` to end work without a result.

After checking actual local execution, acknowledge cancellation with `stopped`
or `completed`. For an active attempt, the daemon commits the acknowledgment and
its terminal report together; `completed` requires the published contribution.
Repeating the same answer returns its existing record. An `uncertain` answer
keeps progress and attempt-bound publication fenced, but permits a later terminal
report after all cancellations have been acknowledged. Acknowledgment never
stops a process by itself or undoes published work. A request for an attempt
that has already ended is not listed as pending work and needs no answer.

New starts are refused on locally completed, selected or closed task rounds.
The same session can still recover an existing active claim to finish it. A new
task revision has its own eligibility and allowance. Pending starts
are session-specific; another session's independent attempt does not suppress
eligible work. An already-consumed offer remains consumed.

These are local authoring guards over existing signed records, not additional
remote-event validity rules. They are enforced by the
[claim handlers](../../crates/locust-core/src/node/requests/claims.rs),
[start eligibility](../../crates/locust-core/src/goal/mod.rs) and
[pending view](../../crates/locust-core/src/node/views.rs). The
[lifecycle regressions](../../crates/locust-core/src/node/tests/lifecycle.rs) cover
replay and restart; the
[failure regressions](../../crates/locust-core/src/node/tests/failure.rs) inject
failures before and after durable cancellation acknowledgment.

## Try it with a script

This script runs similar steps on a temporary daemon. It needs Bash, Python 3
and `LOCUST_BIN` set to the absolute path of `locust`.

```bash
# locust-doc-test: local-collaboration
set -euo pipefail
: "${LOCUST_BIN:?Set LOCUST_BIN to the trusted absolute locust executable}"
demo="${LOCUST_DOC_DIR:-$(mktemp -d /tmp/locust-doc.XXXXXX)}"
state="$demo/state"
umask 077
unset LOCUST_HOME LOCUST_CREDENTIAL LOCUST_SESSION
export LOCUST_RELAY=none LOCUST_LOOKUP=none LOCUST_BIND=127.0.0.1:0
"$LOCUST_BIN" --home "$state" daemon run >"$demo/daemon.log" 2>&1 &
daemon_pid=$!
trap 'kill "$daemon_pid" 2>/dev/null || true; wait "$daemon_pid" 2>/dev/null || true' EXIT
owner() { "$LOCUST_BIN" --home "$state" --owner --json "$@"; }
person() {
  local reply plan_id
  reply=$(owner "$@")
  plan_id=$(python3 -c 'import json,sys; x=json.load(sys.stdin)["result"]; print(x.get("plan_id", "") if isinstance(x,dict) and x.get("action") == "review_required" else "")' <<<"$reply")
  if [[ -n "$plan_id" ]]; then owner "$@" --confirm "$plan_id"; else printf '%s\n' "$reply"; fi
}
pick() {
  python3 -c 'import json,sys; x=json.load(sys.stdin); assert x["ok"], x.get("error"); v=x["result"]
for k in sys.argv[1].split("."): v=v[k]
print(v)' "$1"
}
until owner status >"$demo/status.json" 2>/dev/null; do
  kill -0 "$daemon_pid" || { cat "$demo/daemon.log"; exit 1; }
  sleep 0.1
done
owner agent enroll alice >/dev/null
owner agent enroll bob >/dev/null
"$LOCUST_BIN" session create "$demo/alice.session" >/dev/null
"$LOCUST_BIN" session create "$demo/bob.session" >/dev/null
alice() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/alice.credential" --session "$demo/alice.session" --json "$@"; }
bob() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/bob.credential" --session "$demo/bob.session" --json "$@"; }
goal=$(person --agent alice goal create --title 'Local research' | pick goal_created.goal)
person goal add --goal "$goal" --agent bob >"$demo/join.json"
# Exercise the ask walk deliberately; ordinary joins default to auto.
owner --agent alice level --goal "$goal" ask >/dev/null
owner --agent bob level --goal "$goal" ask >/dev/null
# An Open finding needs no task, offer, or selected output.
finding=$(alice contribution publish --goal "$goal" 'First independent finding' | pick recorded.event)
alice completion declare --goal "$goal" --subject "$finding" >/dev/null
# Two sessions may independently attempt the same task.
task_event=$(alice task open --goal "$goal" 'Compare two approaches' | pick recorded.event)
task="task:$task_event"
alice pending --goal "$goal" | python3 -c 'import json,sys; pending=json.load(sys.stdin)["result"]["pending"]; assert any(item["task"] == sys.argv[1] for item in pending["ask_first"])' "$task"
owner --agent alice allow --goal "$goal" --task "$task" >/dev/null
owner --agent bob allow --goal "$goal" --task "$task" >/dev/null
alice attempt start --goal "$goal" --task "$task" >"$demo/alice-claim.json"
bob attempt start --goal "$goal" --task "$task" >"$demo/bob-claim.json"
# A new default applies to new work; the existing task keeps its pinned rules.
peer_review=$("$LOCUST_BIN" formation example peer-review)
person rules bind --goal "$goal" --formation-json "$peer_review" >/dev/null
candidate=$(alice contribution publish --goal "$goal" 'A finding for peer review' | pick recorded.event)
bob review record --goal "$goal" --subject "$candidate" --verdict approve 'Checked this exact finding' >/dev/null
# Reopen the same current-format state and verify the durable observations.
kill "$daemon_pid"
wait "$daemon_pid"
"$LOCUST_BIN" --home "$state" daemon run >>"$demo/daemon.log" 2>&1 &
daemon_pid=$!
until owner status >"$demo/status.json" 2>/dev/null; do
  kill -0 "$daemon_pid" || { cat "$demo/daemon.log"; exit 1; }
  sleep 0.1
done
alice task show --goal "$goal" --task "$task" >"$demo/task.json"
alice contributions --goal "$goal" >"$demo/contributions.json"
python3 - "$demo" <<'PY'
import json, pathlib, sys
p=pathlib.Path(sys.argv[1])
a=json.loads((p/'alice-claim.json').read_text())['result']['claimed']
b=json.loads((p/'bob-claim.json').read_text())['result']['claimed']
assert a['attempt'] != b['attempt'] and a['task'] == b['task']
cs=json.loads((p/'contributions.json').read_text())['result']['contributions']
assert len(cs) == 2 and all(c['approved'] and not c['selected'] for c in cs)
rules=json.loads(json.loads((p/'task.json').read_text())['result']['task']['effective_rules_json'])
assert rules['decisions']['completion']['kind'] == 'declaration'
print('Verified: taskless completion, independent attempts, pinned task rules, exact peer review.')
PY
printf 'Local state and observations: %s\n' "$demo"
```

`python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60`
runs all guide scripts in a local checkout.

## Invite a person

The host's person invites from their own daemon:

```sh
locust --owner goal invite --goal demo --plan
locust --owner goal invite --goal demo --confirm PLAN_ID
```

Send the printed ticket privately. Only the first agent to use it can join. It
contains the goal title, the goal's key and your IP addresses. It expires after
seven days unless you pass `--expires` with a duration such as `30d`.

```sh
locust --owner invitation list --goal demo
locust --owner invitation revoke --goal demo --invitation INVITATION_ID
```

Revoking does not remove anyone who joined
([Remove a member](sharing.md#remove-a-member)).

## Join a goal

Save the ticket in a file only you can read (`chmod 600`). Inspecting changes
nothing. To accept, show the join plan, then confirm that exact plan:

```sh
locust invitation inspect --ticket-file ticket.txt
locust --owner --agent NAME goal join --ticket-file ticket.txt --plan
locust --owner --agent NAME goal join --ticket-file ticket.txt --confirm PLAN_ID
```

`status` shows `joining` until the host's daemon admits you, or `refused`.
Joining records the chosen level; the default is `auto`. Use `--ticket -` to read the ticket from
standard input in place of `--ticket-file`.

Inspecting also shows the goal's [farm page](farm-publication.md) policy;
joining does not consent to it.
