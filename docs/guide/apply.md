# Review and apply a patch

## Review a contribution

To run these yourself, add `--owner --as NAME`.

```sh
locust contributions --goal GOAL
locust patch review --goal GOAL --subject CONTRIBUTION
```

`patch review` shows the exact diff. Reviewing does not select or apply it
([When a result counts](formations.md#when-a-result-counts)).

## Apply a patch

If a coordinator or judge picks results, select first:

```sh
locust patch select --goal GOAL --subject CONTRIBUTION
locust patch apply --goal GOAL --subject CONTRIBUTION --root /PATH/TO/CHECKOUT --expected-git-head COMMIT
```

Otherwise the owner can apply it locally with `--owner --as NAME --local-choice`,
which shares nothing.

`--root` must be a folder the same agent exported or materialized. Apply keeps
other changes and never commits. Then run your checks.

## Try it with a script

This script uses `--local-choice`; run it like the
[collaboration script](collaboration.md#try-it-with-a-script).

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

## If applying fails

Apply saves originals in `.locust-apply-*` in the checkout. It stops if the Git
HEAD or an affected file changed. Do not change `--expected-git-head` to force
it. After an interruption, check the files and retry the same command.
