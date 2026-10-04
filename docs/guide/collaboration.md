# First collaboration journeys

**Status: implemented development CLI, API 3 / protocol 3.** The local tutorial
below uses two enrolled participants and separate execution sessions on one
isolated daemon. It exercises real CLI requests and persistent state. It does not
establish discovery between machines or native model behavior.

## Two local agents

Use a reviewed local build (`cargo build --locked -p locust`) and Bash/Python 3.
Set `LOCUST_BIN` to its absolute path. The script creates a fresh temporary state
directory, binds networking to loopback with discovery/relays disabled, and stops
only its own foreground daemon at exit. It preserves the state for inspection and
does not configure a service, harness, account or existing checkout.

The two grants permit contributions and reviews in this disposable goal. Execution
is authorized separately for one task. `pick` reads a field from the structured
CLI envelope; credentials and invitations are never printed.

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
pick() {
  python3 -c 'import json,sys; x=json.load(sys.stdin); assert x["ok"], x.get("error"); v=x["result"]
for k in sys.argv[1].split("."): v=v[k]
print(v)' "$1"
}
until owner status >"$demo/status.json" 2>/dev/null; do
  kill -0 "$daemon_pid" || { cat "$demo/daemon.log"; exit 1; }
  sleep 0.1
done
alice_id=$(owner agent enroll alice --manage-goals | pick agent_enrolled.agent)
bob_id=$(owner agent enroll bob --manage-goals | pick agent_enrolled.agent)
"$LOCUST_BIN" session create "$demo/alice.session" >/dev/null
"$LOCUST_BIN" session create "$demo/bob.session" >/dev/null
alice() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/alice.credential" --session "$demo/alice.session" --json "$@"; }
bob() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/bob.credential" --session "$demo/bob.session" --json "$@"; }
goal=$(alice goal create --title 'Local research' | pick goal_created.goal)
ticket=$(alice goal invite --goal "$goal" | pick invited.ticket)
bob goal join --ticket "$ticket" >"$demo/join.json"
for person in "$alice_id" "$bob_id"; do
  owner goal grant --goal "$goal" --agent "$person" --grants \
    '{"administer":true,"contribute":true,"execute":false,"review":true,"select":false,"flow":false,"takeover":false}' >/dev/null
done
# An Open finding needs no task, offer, or selected output.
finding=$(alice contribution publish --goal "$goal" 'First independent finding' | pick recorded.event)
alice completion declare --goal "$goal" --subject "$finding" >/dev/null
# Two sessions may independently attempt the same task.
task_event=$(alice task open --goal "$goal" 'Compare two approaches' | pick recorded.event)
task="task:$task_event"
for person in "$alice_id" "$bob_id"; do
  owner task authorize --goal "$goal" --task "$task" --agent "$person" >/dev/null
done
alice attempt start --goal "$goal" --task "$task" >"$demo/alice-claim.json"
bob attempt start --goal "$goal" --task "$task" >"$demo/bob-claim.json"
# A new default applies to new work; the existing task keeps its pinned rules.
rules=$(alice goal status --goal "$goal" | pick goal_status.current_rules)
peer_review=$("$LOCUST_BIN" formation example peer-review)
alice rules bind --goal "$goal" --expected "$rules" --formation-json "$peer_review" >/dev/null
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

An attempt start records a local claim; this script launches no agent process.
The claim's `attempt` and `generation` must accompany submissions that name the
attempt. A terminal `attempt report --status completed` reports that occurrence;
it does not substitute for the contribution's completion rule. Inspect `pending`
and `task show` to see the rules and obligations for the current caller.

The repository checks this exact fenced recipe with
`python3 scripts/check_documentation.py --binary /absolute/locust --timeout 60`.
The explicit timeout is a test watchdog, not a runtime work limit.

## Invite a person

An administrator runs `goal invite --goal GOAL` and shares the returned ticket
through a channel they deliberately choose. Treat the ticket as a capability;
keep it out of command history, public pages, source control and diagnostic
transcripts. The recipient runs `invitation inspect --ticket -` with the ticket
on standard input, or `invitation inspect --ticket-file /private/invitation`.
A ticket file must be an owner-only regular file with mode `0600` or `0400`.
Inspection works offline and does not redeem the invitation or change membership.

The preview verifies the administrator's signature over the goal, title,
endpoint/contact hints, capability, expiry and whole-goal sharing boundary. The
title is administrator-signed presentation, and the key fingerprint does not
verify a person's identity. Inspection cannot establish current issuer
availability, revocation or admission. Available history and shared goal content
become readable on admission; local files and private chats are not shared
automatically.

To accept, use `locust --owner invitation join --principal NAME --ticket-file
/private/invitation --review REVIEW_IDENTIFIER`, where `NAME` is an existing
enrolled local principal and the full review identifier comes from inspecting
that exact ticket. Standard input via `--ticket -` also works. A changed ticket
requires another review. To decline, take no action. A remote join may initially
report `joining`; `locust --owner status` shows joining/refused state and `goal status`
becomes readable when signed admission arrives. Retrying the same reviewed ticket
recovers the pending or admitted result. Expired or refused invitations require a
fresh invitation; an unresolved pending join rejects ticket substitution. Joining grants no
local execution, provider spending or workspace permission.

The issuer can run `locust --owner invitation list --goal GOAL` to see pending,
expired, revoked and redeemed invitations without their capabilities. Use
`locust --owner invitation revoke --goal GOAL --invitation IDENTIFIER` to revoke
an unused invitation. Revocation survives restart and repeated requests are
idempotent. A redeemed invitation requires `member remove` to end membership;
neither action retracts copies already received. Invitation operations are
excluded from the model tool surface; an otherwise authorized CLI/API caller
retains its existing authority.

Before sending, identify the recipient, goal and shared content. Topics and roles
do not create private channels. Use a separate goal with separate membership for
a confidential subgroup; export only chosen inputs and review returned material
under the parent goal's rules. Invitations with endpoint hints were exercised
locally. Key-only multicast discovery is currently failing on the qualification
host; see [availability](status.md) before planning a remote journey.

## Share a code snapshot

Choose an exact repository commit and review the files before export. The current
workspace exporter captures that Git snapshot; it does not export arbitrary
unstaged files. Choose a commit containing only the intended shared material.

```sh
locust workspace export --goal GOAL --root /absolute/repository --commit COMMIT
locust workspace materialize --goal GOAL --manifest MANIFEST --destination /absolute/new-workspace
```

Use the returned manifest as a named task input declared by the formation, for
example `task open --goal GOAL --inputs '{"workspace":"MANIFEST"}' 'Review the snapshot'`.
Undeclared input names are refused. Exporting material does not execute it or
change the recipient's main checkout. A patch names its exact base and artifact;
a signed check attributes a claim, not independent test execution.

Read [completion](completion.md) before choosing an output and
[local patch application](apply.md) before changing a checkout.
