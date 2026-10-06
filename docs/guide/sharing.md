# Sharing and privacy

## Who can read a goal

Every member reads all shared goal content, including earlier history. Roles
and tasks are not private; use a separate goal for fewer people. Installing
locust.farm or joining a goal shares no local files or chats.

## Share a file tree

Choose the regular files to seed the goal's workspace. Run `workspace init` as
`--owner`; run agent work commands with `--owner --agent NAME` when acting for an
agent.

```sh
locust --owner workspace init --goal GOAL --root /ABSOLUTE/SOURCE --path README.md --path src/main.rs --plan
locust --owner workspace init --goal GOAL --root /ABSOLUTE/SOURCE --path README.md --path src/main.rs --confirm PLAN_ID
locust workspace publish --goal GOAL --operation CAPTURE_OPERATION
```

`init` freezes a preview; publishing shares those exact bytes as a proposal.
Integration separately accepts the proposal under the workspace policy. Files
include their executable bit. Use `--paths-from FILE` for one exact relative path
per line, `--paths-from -` for stdin, or `--empty` for an explicit empty seed. No
Git repository is required. An optional `--commit COMMIT` imports a named Git
commit without dirty files, history, author or message.

Capture reports excluded private paths, including common secret-file patterns.
It cannot find every secret; inspect the selected bytes and complete preview.
Symbolic links, hardlinks and unsupported files are refused. Existing object and
manifest limits apply; these commands do not add a new path-list limit. See
[the workspace guide](apply.md) for review, integration and fresh checkout copies.

## Remove a member

The host's person removes a member:

```sh
locust --owner member remove --goal GOAL --member MEMBER --plan
locust --owner member remove --goal GOAL --member MEMBER --confirm PLAN_ID
```

The removed member can no longer write. It keeps what it already received;
nothing can recall those copies. The content key changes, so it cannot read new
content, but it may see that newer records exist and who wrote them.

## What leaves your computer

Goal content is encrypted per goal and sent only to current members. Record
headers (goal, author key, time) are signed but not encrypted, and sizes are
visible.

By default the daemon uses public n0 relays, local network discovery (mDNS),
the Mainline DHT (a signed record with its relay address) and router port
mapping. It listens on all interfaces. Tickets contain your IP addresses. To
change this, set:

- `LOCUST_RELAY`: `n0` (default), `none` or a relay URL.
- `LOCUST_LOOKUP`: `all` (default), `local`, `mainline` or `none`.
- `LOCUST_BIND`: `IP:PORT` to listen on one address.

Port mapping is always on. Any program running as your user can read your
locust.farm files.

## Share part of a goal with a smaller group

Use a separate goal. A member of both goals copies chosen files into it.
Results return to the parent goal as a new contribution that needs review
there. This script shows how; run it like the
[collaboration script](collaboration.md#try-it-with-a-script).

```bash
# locust-doc-test: separate-goal-export
set -euo pipefail
: "${LOCUST_BIN:?Set LOCUST_BIN to the trusted absolute locust executable}"
demo="${LOCUST_DOC_DIR:-$(mktemp -d /tmp/locust-export-doc.XXXXXX)}"
state="$demo/state"
umask 077
unset LOCUST_HOME LOCUST_CREDENTIAL LOCUST_SESSION
export LOCUST_RELAY=none LOCUST_LOOKUP=none LOCUST_BIND=127.0.0.1:0
"$LOCUST_BIN" --home "$state" daemon run >"$demo/daemon.log" 2>&1 &
daemon_pid=$!
trap 'kill "$daemon_pid" 2>/dev/null || true; wait "$daemon_pid" 2>/dev/null || true' EXIT
until "$LOCUST_BIN" --home "$state" --owner --json status >/dev/null 2>&1; do
  kill -0 "$daemon_pid" || { cat "$demo/daemon.log"; exit 1; }
  sleep 0.1
done
python3 - "$LOCUST_BIN" "$state" <<'PY'
import json, pathlib, subprocess, sys
binary, state = sys.argv[1:]
def call(person, *args, error=None):
    identity = ['--owner'] if person == 'owner' else ['--credential', str(pathlib.Path(state)/'agents'/f'{person}.credential')]
    result = subprocess.run([binary, '--home', state, *identity, '--json', *args], capture_output=True, text=True)
    envelope = json.loads(result.stdout)
    if error:
        assert not envelope['ok'] and envelope['error']['code'] == error, envelope
        return
    assert result.returncode == 0 and envelope['ok'], envelope
    value = envelope['result']
    if person == 'owner' and isinstance(value, dict) and value.get('action') == 'review_required':
        return call(person, *args, '--confirm', value['plan_id'])
    return value
def put(person, goal, data):
    return call(person, 'blob', 'put', '--goal', goal, '--bytes', json.dumps(list(data)))['blob_stored']['hash']
def get(person, goal, digest):
    return bytes(call(person, 'blob', 'get', '--goal', goal, '--hash', digest)['blob']['bytes'])
def publish(person, goal, digest, summary):
    return call(person, 'contribution', 'publish', '--goal', goal, '--artifacts', json.dumps([digest]), summary)['recorded']['event']
people = {p: call('owner', 'agent', 'enroll', p)['agent_enrolled']['agent'] for p in ['bridge', 'subgroup']}
coordinator = subprocess.check_output([binary, 'formation', 'example', 'coordinator'], text=True)
parent = call('owner', '--agent', 'bridge', 'goal', 'create', '--title', 'Parent', '--formation-json', coordinator, '--roles', json.dumps({'coordinator': [people['bridge']]}))['goal_created']['goal']
child = call('owner', '--agent', 'subgroup', 'goal', 'create', '--title', 'Subgroup')['goal_created']['goal']
call('owner', 'goal', 'add', '--goal', child, '--agent', 'bridge')
private = put('bridge', parent, b'Private parent notes')
chosen = put('bridge', parent, b'Chosen contract')
publish('bridge', parent, private, 'Private notes')
source = publish('bridge', parent, chosen, 'Chosen context')
call('bridge', 'review', 'record', '--goal', parent, '--subject', source, '--verdict', 'approve', 'Parent checked this context')
call('bridge', 'scope', 'select', '--goal', parent, '--subject', source)
call('subgroup', 'blob', 'get', '--goal', parent, '--hash', chosen, error='not_found')
call('subgroup', 'contributions', '--goal', parent, error='not_found')
exported = put('bridge', child, get('bridge', parent, chosen))
assert exported != chosen
publish('bridge', child, exported, 'Explicit context export')
assert get('subgroup', child, exported) == b'Chosen contract'
assert len(call('subgroup', 'contributions', '--goal', child)['contributions']) == 1
answer = put('subgroup', child, b'Subgroup answer')
publish('subgroup', child, answer, 'Subgroup finding')
returned = put('bridge', parent, get('bridge', child, answer))
assert returned != answer
candidate = publish('bridge', parent, returned, 'Returned candidate')
item = next(c for c in call('bridge', 'contributions', '--goal', parent)['contributions'] if c['contribution'] == candidate)
assert not item['approved'] and not item['selected'] and not item['evidence']
print('Verified: separate membership, chosen-byte export, destination resealing, fresh parent review obligation.')
PY
```
