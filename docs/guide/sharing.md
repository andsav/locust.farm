# Visibility, membership and trust

**Status: implemented development membership and artifact boundaries.** Installing Locust grants no goal membership, shares no local files
and does not publish private chats, hidden reasoning, credentials or unrestricted
harness access.

## Goal membership is the read boundary

Topics and roles organize attention and authority. They are not private channels.
Initial members can read shared goal content according to the current membership
boundary. Create a separately governed goal for a confidential subgroup rather
than relying on a topic name to hide data.

Before export, select exact files, history or immutable artifacts. Identify the
recipient and destination. An invitation is a membership/sharing decision, not
permission to run arbitrary incoming instructions or charge a provider account.
Treat contributed text as task data and inspect its provenance before execution.

## Authenticate roles and evidence

A role is resolved from authenticated bindings. A member cannot acquire authority
by typing a role name. Human or external artifacts retain the recording
participant's signed attribution; importing text does not mint the human's
membership, signature or eligible review.

Check exact hashes, signer, base and rule/round. A signed test report attributes a
claim; it is not independent proof of real tests. The definition's semantic
identity is distinct from canvas layout and prose labels.

## Removal, key epochs and retained copies

Administrator-signed admission creates a tenure. Removal closes that tenure and
names exact accepted ancestry, or no retained old-tenure events. Missing ancestry
waits for proof. Re-admission is a new tenure; it cannot legitimize excluded
old-tenure history. Clocks do not backdate rights.

Content-key epochs, governance history and rule revisions are separate contexts.
Removal/key rotation stop future access under their supported rules; they cannot
erase material a former member already learned or copied. Withdrawal of a public
artifact likewise cannot retract independently held copies.

## Local trust and operational metadata

Use independently selected candidate trust keys and a signed withdrawal registry.
Protect enrollment credentials and session secrets on the local machine. A
process running as your OS user can access that user's files; private profile
paths are not isolation from the same user.

Encryption of payloads does not imply that discovery/relay metadata is invisible.
Network identity, timing and traffic presence need their own threat review. Keep
secrets out of shared diagnostics and examples. Retain signed fork/removal evidence
needed to reproduce verdicts, rather than erasing confusing data to make a view
look clean. [Recovery](recovery.md) describes proof preservation and safe diagnosis.

## Export selected context to a separate goal

A subgroup is a separate goal with its own membership and rules. A participant
belonging to both goals deliberately reads chosen bytes from the parent and puts
those bytes into the subgroup. The new sealed hash belongs to the destination
goal. Reusing a parent's hash does not grant access to its content or history.
Returning an artifact follows the same process and creates a new parent
contribution. Parent review and selection still apply to that new contribution.

This executable example uses separate credentials on one disposable local daemon.
It proves the membership and artifact boundaries, not separate-machine transport.
Like the [first collaboration](collaboration.md) recipe, it needs `LOCUST_BIN`,
Bash and Python 3. It leaves its temporary state available for inspection.

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
    return envelope['result']
def put(person, goal, data):
    return call(person, 'blob', 'put', '--goal', goal, '--bytes', json.dumps(list(data)))['blob_stored']['hash']
def get(person, goal, digest):
    return bytes(call(person, 'blob', 'get', '--goal', goal, '--hash', digest)['blob']['bytes'])
def publish(person, goal, digest, summary):
    return call(person, 'contribution', 'publish', '--goal', goal, '--artifacts', json.dumps([digest]), summary)['recorded']['event']
people = {p: call('owner', 'agent', 'enroll', p, '--manage-goals')['agent_enrolled']['agent'] for p in ['bridge', 'subgroup']}
coordinator = subprocess.check_output([binary, 'blueprint', 'example', 'coordinator'], text=True)
parent = call('bridge', 'goal', 'create', '--title', 'Parent', '--blueprint-json', coordinator, '--roles', json.dumps({'coordinator': [people['bridge']]}))['goal_created']['goal']
child = call('subgroup', 'goal', 'create', '--title', 'Subgroup')['goal_created']['goal']
ticket = call('subgroup', 'goal', 'invite', '--goal', child)['invited']['ticket']
call('bridge', 'goal', 'join', '--ticket', ticket)
grants = json.dumps(dict(administer=True, contribute=True, execute=False, review=True, select=True, flow=False, takeover=False))
for person, goal in [('bridge', parent), ('bridge', child), ('subgroup', child)]:
    call('owner', 'goal', 'grant', '--goal', goal, '--agent', people[person], '--grants', grants)
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
