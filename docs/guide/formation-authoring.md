# Write a formation

## Check a formation without the daemon

No daemon is needed. `-` reads standard input; `--json` adds the response
envelope.

```sh
locust formation contract
locust formation schema
locust formation examples
locust formation example peer-review > peer-review.json
locust formation validate peer-review.json
locust formation explain peer-review.json
locust formation normalize peer-review.json
locust formation diff open.json peer-review.json
```

## Read the problems Locust reports

Each problem has a `code`, `phase`, JSON Pointer `path`, `message` and suggested
`correction`. Locust refuses other schema versions.

## Build your own from a preset

Start from `locust formation example NAME > team.json`, edit it, and run
`validate` and `explain` until the rules read right. `diff` against the preset
shows what changed. Keep secrets out: every goal member can read the formation.

## Save a draft and publish it

An author credential (`locust --owner author enroll NAME`) can draft and
publish but not create goals. Each draft change and `formation publish` names
the revision you expect; if the draft changed, Locust refuses and returns it.
This script drafts and publishes:

```bash
# locust-doc-test: private-authoring
set -euo pipefail
: "${LOCUST_BIN:?Set LOCUST_BIN to the trusted absolute locust executable}"
demo="${LOCUST_DOC_DIR:-$(mktemp -d /tmp/locust-author.XXXXXX)}"
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
owner author enroll designer >/dev/null
author() { "$LOCUST_BIN" --home "$state" --credential "$state/authors/designer.credential" --json "$@"; }
"$LOCUST_BIN" formation example open >"$demo/open.json"
"$LOCUST_BIN" formation validate "$demo/open.json" >"$demo/inspection.json"
author formation draft create --id research --expected-revision 0 - <"$demo/open.json" >"$demo/draft.json"
revision=$(pick formation_draft.revision <"$demo/draft.json")
source_hash=$(pick formation_draft.source_hash <"$demo/draft.json")
author formation publish --draft research --id research-v1 \
  --expected-revision "$revision" --expected-source-hash "$source_hash" >"$demo/publication.json"
# A stale expected revision must not replace the saved source.
if author formation draft update --id research --expected-revision 0 '{}' >"$demo/conflict.json"; then
  echo 'Unexpected stale edit success'; exit 1
fi
# An author credential cannot instantiate a goal or acquire work authority.
if author goal create --title 'Must be refused' >"$demo/denied.json"; then
  echo 'Unexpected author goal permission'; exit 1
fi
python3 - "$demo" <<'PY'
import json, pathlib, sys
p=pathlib.Path(sys.argv[1])
read=lambda name: json.loads((p/name).read_text())
draft=read('draft.json')['result']['formation_draft']
publication=read('publication.json')['result']['formation_publication']
assert publication['draft_revision'] == draft['revision']
assert publication['source_hash'] == draft['source_hash']
conflict=read('conflict.json')['error']
assert conflict['code'] == 'conflict'
assert conflict['details']['current_draft'] == draft
assert read('denied.json')['ok'] is False
print('Verified: exact private publication, recoverable edit conflict, author-only authority.')
PY
printf 'Private catalog and observations: %s\n' "$demo"
```

## Start a goal with your formation

```sh
locust goal create --title TITLE --formation review-panel --roles '{"reviewer":["YOUR_KEY"]}'
```

For your own formation, use `--formation-json "$(cat team.json)"`. `--roles`
maps roles to public keys (`locust --owner --json status` lists them); `--inputs`
maps inputs to file hashes. Creating goals needs `agent enroll --manage-goals`.
You become the administrator and only member, so roles can name only you until
others join and you run [rules bind](formations.md#changing-the-rules).

## Write one with your agent

Tell your agent who takes part, who starts work, when a result counts and
whether one result is picked. Ask it to read `locust formation contract`, start
from an example, validate, explain, and report what the schema cannot
express. Have it ask before publishing.

## Use the editor on locust.farm

The editor at https://locust.farm/formations (password-protected preview)
builds a formation without JSON: six ways of working, four questions, roles,
steps and task types. A TypeScript copy of Locust's checks runs in your browser,
held to Locust's results by shared test cases. Work stays in the browser.

The editor copies one prompt. It has your agent check the formation and, if you
choose, save a private draft and ask before publishing. It never starts a goal.
[The prompt contract](../formation-prompt.md) has its text.
