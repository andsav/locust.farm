# Apply a chosen patch

**Status: implemented development workspace flow.** A distributed selection and a
change to a local checkout are independent actions. The normal apply path checks
that the exact contribution is selected in its scope. An explicit local owner
choice also supports unselected Open findings without inventing a shared head.

## Select an exact output

Inspect the contribution ID, base snapshot, patch artifact, provenance and pinned
rule/round. A review applies to that candidate alone. Several candidates may
qualify; the rule's selection authority can choose one where selection is
configured. `scope select --goal GOAL --subject CONTRIBUTION` selects the initial
output. Later changes supply the current decision event with `--expected`.
`patch review --subject CONTRIBUTION` derives the signed patch and base; `--patch` remains available for inspecting an unpublished patch.
`patch select --subject CONTRIBUTION` additionally checks the subject's exact patch artifact.

A local owner can choose an effective, unselected contribution for their own
checkout with `patch apply --local-choice`. This requires authenticated `--owner`
and `--as PRINCIPAL`; an agent cannot activate it just by adding the flag. It
creates no review, approval or replicated selection.

## Review the local checkout

Identify the absolute workspace and branch. Compare its base with the contribution
and inspect staged, unstaged and untracked work. The apply guard preserves
unrelated files and refuses conflicting changes or a stale base. It never grants
permission to force-reset a checkout.

Use `patch review --goal GOAL --patch PATCH` to inspect the exact artifact and
its declared changes. A goal administrator or remote reviewer cannot authorize
arbitrary filesystem mutation on your machine. Inspect the actual target before
using either the selected-output or local-choice route.

## Apply and verify

For a selected contribution, use:

```sh
locust patch apply --goal GOAL --subject CONTRIBUTION \
  --root /absolute/checkout --expected-git-head COMMIT
```

Here `CONTRIBUTION` identifies the signed event; `PATCH` identifies the immutable
patch package; `MANIFEST` identifies its exported base. `COMMIT` is the expected
Git HEAD of the target. These are different identities. After applying, inspect
files and run the project's relevant checks. The workspace's `integrated` field
records the locally applied artifact; it is not a goal-wide accepted result or
independent test evidence. Applying does not commit or push.

The following executable example creates only disposable Git repositories and a
fresh local daemon. It proves the Open route leaves the contribution unselected
and unapproved while changing the intended file. Use the trusted `LOCUST_BIN`
from [the first tutorial](collaboration.md); Bash, Python 3 and Git are required.

```bash
# locust-doc-test: open-patch-application
set -euo pipefail
: "${LOCUST_BIN:?Set LOCUST_BIN to the trusted absolute locust executable}"
demo="${LOCUST_DOC_DIR:-$(mktemp -d /tmp/locust-patch.XXXXXX)}"
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
person=$(owner agent enroll maker --manage-goals | pick agent_enrolled.agent)
agent() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/maker.credential" --json "$@"; }
goal=$(agent goal create --title 'Open patch example' | pick goal_created.goal)
owner goal grant --goal "$goal" --agent "$person" --grants \
  '{"administer":true,"contribute":true,"execute":false,"review":false,"select":false,"flow":false,"takeover":false}' >/dev/null
mkdir "$demo/source"
git -C "$demo/source" init -q
git -C "$demo/source" config core.hooksPath /dev/null
printf 'before\n' >"$demo/source/code.txt"
git -C "$demo/source" add code.txt
git -C "$demo/source" -c user.name='Locust example' -c user.email=example@example.invalid -c commit.gpgSign=false commit -qm 'Example base'
commit=$(git -C "$demo/source" rev-parse HEAD)
base=$(agent workspace export --goal "$goal" --root "$demo/source" --commit "$commit" | pick manifest)
agent workspace materialize --goal "$goal" --manifest "$base" --destination "$demo/worker" >/dev/null
printf 'after\n' >"$demo/worker/code.txt"
agent patch create --goal "$goal" --base "$base" --root "$demo/worker" --path code.txt >"$demo/patch.json"
patch=$(pick contribution_id <"$demo/patch.json")
subject=$(agent contribution publish --goal "$goal" --base "$base" --patch "$patch" 'A taskless patch' | pick recorded.event)
agent patch review --goal "$goal" --patch "$patch" >"$demo/review.json"
if agent patch apply --goal "$goal" --subject "$subject" --root "$demo/source" --expected-git-head "$commit" >"$demo/unselected.json"; then
  echo 'Unexpected unselected apply success'; exit 1
fi
owner --as "$person" patch apply --goal "$goal" --subject "$subject" \
  --root "$demo/source" --expected-git-head "$commit" --local-choice >"$demo/applied.json"
agent contributions --goal "$goal" >"$demo/contributions.json"
python3 - "$demo" <<'PY'
import json, pathlib, sys
p=pathlib.Path(sys.argv[1])
assert (p/'source/code.txt').read_text() == 'after\n'
cs=json.loads((p/'contributions.json').read_text())['result']['contributions']
assert len(cs) == 1 and not cs[0]['approved'] and not cs[0]['selected']
print('Verified: explicit local choice applies exact bytes without distributed selection.')
PY
printf 'Repositories and observations: %s\n' "$demo"
```

## Recovery evidence

After interruption, inspect both the local application receipt and actual files
before retrying. Keep the same exact subject, artifact and base. A timeout is an
uncertain outcome. If the checkout changed concurrently or the receipt and files
disagree, preserve the workspace and reconcile it explicitly; do not change the
expected base simply to bypass a refusal.

The workspace library's tests cover application conflicts and idempotent retry.
The current CLI qualification also covers reviewed selection and preservation of
unrelated work. These are local checks; they do not prove that a remote execution
stopped or that a repository change was committed. [Recovery](recovery.md) and
[completion](completion.md) describe those separate observations.
