# Work on the shared tree

Status: implemented command surface; source tests exercise ordinary directories,
signed replay and recovery separately. Two-host and real-agent qualification of
this new protocol is still pending.

A workspace-enabled goal has an accepted revision. Each member works in an
ordinary directory pinned to a revision. Publishing proposes a tree; integration
accepts it; updating copies that accepted result into a local checkout. These are
separate operations. No Git repository is required.

To run an agent's work commands yourself, add `--owner --agent NAME`. Use
`--owner` alone for `workspace init`. The goal's formation and local level
determine who can publish, review and integrate.

## Start and inspect a tree

Choose every seed file explicitly, or use `--paths-from FILE` with one relative
path per line. `--paths-from -` reads the list from stdin.

```sh
locust --owner workspace init --goal GOAL --root /ABSOLUTE/SOURCE --path src/main.rs --path README.md --plan
locust --owner workspace init --goal GOAL --root /ABSOLUTE/SOURCE --path src/main.rs --path README.md --confirm PLAN_ID
locust workspace publish --goal GOAL --operation CAPTURE_OPERATION
locust workspace pending --goal GOAL
```

`init` is the host's command to prepare the workspace policy and freeze a
preview. By default the host's agent records accepted changes; an explicit
formation workspace policy can name another member. The shared files follow the goal's
completion rule unless the formation or `--completion` gives them their own rule.
The first files need no approval. Policy setup needs the host's authority. Existing workspace policy is
not silently replaced. When the host changes the goal's rules with `rules bind`,
the tree moves to their completion rule too, unless the new formation supplies
an explicit workspace rule.
`--empty` seeds an empty tree. `--commit COMMIT --root /ABSOLUTE/REPO` optionally
imports the regular files of a named Git commit.

Read the preview before publishing. `publish` uses the stored candidate even if
the source files subsequently change. `init --publish` combines capture and
publication, but still requires integration. The first files count when posted,
then the host's agent accepts the exact proposal:

```sh
locust workspace integrate --goal GOAL --proposal PROPOSAL --expected-empty
locust workspace head --goal GOAL
locust workspace tree --goal GOAL --revision REVISION
locust workspace read --goal GOAL --revision REVISION --path src/main.rs
```

Later proposals require the evidence their completion rule names. `head` reports authority
and content readiness separately: an accepted revision can still have missing
files or keys. Tree listing supports `--path`, `--after-path` and an explicit
`--limit`; file reads support `--offset` and `--length`.

## Work in an ordinary directory

```sh
locust --owner --agent NAME workspace connect --goal GOAL --revision REVISION --folder /ABSOLUTE/NEW/FOLDER --plan
locust --owner --agent NAME workspace connect --goal GOAL --revision REVISION --folder /ABSOLUTE/NEW/FOLDER --confirm PLAN_ID
locust workspace bind --goal GOAL --checkout CHECKOUT
locust workspace status --goal GOAL --checkout CHECKOUT
locust workspace propose --goal GOAL --checkout CHECKOUT --path new-file.txt
locust workspace publish --goal GOAL --operation CAPTURE_OPERATION
```

The person's connect command copies files into the folder they named and
records its exact base. An agent can request its own fresh daemon-created folder
with `checkout register --goal GOAL --checkout ID`, using a fresh 16-byte hex ID
and an optional `--revision`, `--task` and `--attempt`. The response gives the
folder root and base. `bind` explicitly associates this authenticated session
with that checkout, so context
and pending work report it. Propose
captures modifications and deletions of managed files, plus explicitly selected
additions. It does not discover additions through Git tracking or ignore rules.
Use `--only` to capture just the supplied paths. Omitted changes remain local.
Private paths are excluded and reported; exclusions are a precaution, so inspect
the complete preview for secrets. No directory watcher publishes changes.

## Review, compose and integrate

```sh
locust workspace review --goal GOAL --proposal PROPOSAL --destination /ABSOLUTE/NEW/REVIEW
locust workspace compose --goal GOAL --head REVISION --source FIRST_PROPOSAL --source SECOND_PROPOSAL
locust workspace integrate --goal GOAL --proposal COMBINED_PROPOSAL --expected-head REVISION
```

Review shows the exact candidate and optionally copies it into a fresh review
directory. Run checks there yourself; Locust does not execute supplied commands.
Composition creates a new preview from the sources in the stated order. Conflicts
refuse composition. Publish the preview, then supply completion or review evidence
for that exact combined proposal before integration. An approval of an earlier
source does not approve new combined bytes. Integration checks the expected head
and workspace epoch, formation eligibility, evidence and the local level.

For an accepted tree whose parent content is unavailable, a complete replacement
can be captured with `propose --replace --parent REVISION --checkout CHECKOUT`
and explicit selected paths, or `--empty`. This produces a new proposal and does
not bypass integration authority.

## Update and recover

```sh
locust workspace update --goal GOAL --checkout CHECKOUT --revision REVISION
locust workspace recover --goal GOAL --operation OPERATION
```

Update compares the stored base, current local files and requested accepted tree.
It preserves compatible unpublished edits, reports them as dirty, and explicitly
adopts additions already equal to the target. Conflicting edits or layout
collisions stop before file mutation. An accepted head moving elsewhere does not
automatically retarget a running checkout.

Before changing files, the CLI saves a durable plan, original copies and staged
replacements in a private sibling recovery directory on the same filesystem,
outside all registered managed roots. The daemon stores the exact operation and
recovery identity. Updates serialize on the checkout directory and refuse
symlinks, hardlinks and changed preimages. This is recoverable per-file work,
not an atomic multi-file replacement.

After an interrupted or uncertain operation, use its reported operation ID with
`recover`. Recovery verifies the recorded identity and filesystem state; unknown
states or outside edits require inspection and are never silently overwritten.
Originals remain available in the recovery directory. A completed update records
whether its target still belonged to accepted history when the binding completed.
See the [implementation contract](../workspace.md) for ownership and proof limits.

## Try the complete local loop

This recipe needs Bash, Python 3 and `LOCUST_BIN` set to the absolute path of a
trusted local `locust` executable. It starts a fresh loopback-only daemon and uses
one member with the default peer-review rule. Its results count when posted,
because it is the goal's only member. It launches no models.

```bash
# locust-doc-test: shared-workspace-loop
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
"$LOCUST_BIN" session create "$demo/alice.session" >/dev/null
alice() { "$LOCUST_BIN" --home "$state" --credential "$state/agents/alice.credential" --session "$demo/alice.session" --json "$@"; }
goal=$(person --agent alice goal create --title 'Shared tree example' | pick goal_created.goal)
mkdir "$demo/source"
printf 'base\n' >"$demo/source/app.txt"
printf 'original\n' >"$demo/source/local.txt"
seed_operation=$(person workspace init --goal "$goal" --root "$demo/source" --path app.txt --path local.txt | pick operation.id)
seed_proposal=$(alice workspace publish --goal "$goal" --operation "$seed_operation" | pick workspace_operation.state.recorded.event)
seed_revision=$(alice workspace integrate --goal "$goal" --proposal "$seed_proposal" --expected-empty | pick workspace_operation.state.recorded.event)
checkout=$(person --agent alice workspace connect --goal "$goal" --revision "$seed_revision" --folder "$demo/checkout" | pick checkout.id)
alice workspace bind --goal "$goal" --checkout "$checkout" >/dev/null
printf 'accepted change\n' >"$demo/checkout/app.txt"
printf 'unpublished edit\n' >"$demo/checkout/local.txt"
printf 'unrelated private note\n' >"$demo/checkout/note.txt"
# Only app.txt enters this proposal; other work remains local.
operation=$(alice workspace propose --goal "$goal" --checkout "$checkout" --only --path app.txt | pick operation.id)
proposal=$(alice workspace publish --goal "$goal" --operation "$operation" | pick workspace_operation.state.recorded.event)
alice workspace review --goal "$goal" --proposal "$proposal" >"$demo/review.json"
revision=$(alice workspace integrate --goal "$goal" --proposal "$proposal" --expected-head "$seed_revision" | pick workspace_operation.state.recorded.event)
# Restore this path to its base to demonstrate receiving the accepted change.
printf 'base\n' >"$demo/checkout/app.txt"
alice workspace update --goal "$goal" --checkout "$checkout" --revision "$revision" >"$demo/update.json"
alice workspace status --goal "$goal" --checkout "$checkout" >"$demo/checkout-status.json"
python3 - "$demo" "$revision" <<'PY'
import json, pathlib, sys
p = pathlib.Path(sys.argv[1])
checkout = p / 'checkout'
assert (checkout / 'app.txt').read_bytes() == b'accepted change\n'
assert (checkout / 'local.txt').read_bytes() == b'unpublished edit\n'
assert (checkout / 'note.txt').read_bytes() == b'unrelated private note\n'
assert not (checkout / '.git').exists() and not (p / 'source/.git').exists()
status = json.loads((p / 'checkout-status.json').read_text())['result']
assert status['checkout']['base_revision'] == sys.argv[2]
assert status['dirty_paths'] == ['local.txt'] and status['untracked_paths'] == ['note.txt']
assert json.loads((p / 'update.json').read_text())['result']['target_in_lineage_at_completion']
print('Verified: explicit seed, exact proposal acceptance, ordinary checkout update and preserved local work.')
PY
```

`python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60`
runs this recipe and the other executable guide examples.
