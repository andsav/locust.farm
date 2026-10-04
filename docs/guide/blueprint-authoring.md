# Blueprint authoring

**Status: implemented development authoring and private catalog.** Offline
inspection needs no daemon. Authenticated drafts and immutable publication use
the daemon; instantiating a goal is a separate authorized operation.

## Discover the contract

Fetch the documentation inventory for this development build. Record its source
commit and blueprint schema version, then read the matching schema and checked
examples. The source and schema are separate identities: a source build is not a
published software release.

Do not invent operation names or fields from a diagram or a prose example. The
schema exported by the Rust definition model is the shape contract. Canonical
example files must be checked by that same core validator.

## Offline commands

The development CLI contract provides these commands. They operate on definitions
without a daemon, account or local state. Use a locally built development binary;
these commands do not imply that a downloadable release exists.

```sh
locust blueprint contract
locust blueprint schema
locust blueprint examples
locust blueprint example peer-review > peer-review.json
locust blueprint validate peer-review.json
locust blueprint explain peer-review.json
locust blueprint normalize peer-review.json
locust blueprint example open > open.json
locust blueprint diff open.json peer-review.json
```

`validate`, `explain` and `normalize` also accept `-` for JSON on standard input.
Schema, example and normalization commands emit raw JSON by default. `--json`
uses the CLI's structured result envelope. Inspection reports include validity,
diagnostics, normalized definition, semantic hash and effective-rule explanation.
Each diagnostic identifies a code, phase, path, message and suggested correction.
`diff BEFORE AFTER` compares normalized definitions from two files, returning
both semantic hashes and exact JSON Pointer changes. Formatting/default expansion
alone preserves equivalence. An invalid side returns its diagnostics and no
equivalence claim; the comparison does not evaluate live bindings or permissions.

The example names are `open`, `coordinator`, `peer-review`, `independent-attempts`,
`review-panel` and `pipeline`. Source tests and export parity checks qualify the offline surface.
The runtime has separate signed-event and daemon acceptance tests. The [generated reference](schema-reference.md)
provides exact field and operation discovery.

## Draft and check

Start from a matching example and change only the rules needed for your intended
organization. Preserve explicit role and authority references. Keep credentials,
invitation tickets, account identifiers and private transcripts out of definitions
intended for sharing.

Use the offline validator to check a definition's current supported semantics.
Use the effective-rule explainer to inspect the rules it resolves. A failure is a
reason to revise the definition or clarify the intended behavior, not to bypass
validation or fall back to an older format.

## Shape, semantics and readiness

| Check | What it establishes | What it cannot establish |
| --- | --- | --- |
| Schema validation | JSON has the supported field and type shape | That all rules are semantically supported |
| Core semantic validation | References and rules satisfy the implemented authoring contract | Local permissions, actor availability or live bindings |
| Effective-rule explanation | How the offline model resolves declared rules | That a runtime action can execute or finalize |
| Contextual readiness | Daemon checks exact bindings, membership, rules and grants for the requested action | Process start, independent testing, provider permission or remote availability |

A structurally valid definition is not proof of unique decision authority or
runtime readiness. A successful offline explanation does not publish a definition,
create an organization instance, grant access or execute an assignment.

## Review before runtime use

Review the effective rules alongside the draft. Keep the checked file and its
source/schema identity together. A private author credential can create and publish templates without gaining
goal membership or execution authority. The short recipe below creates one
private Open template and checks a stale edit refusal. It uses the same local
build prerequisites as [the collaboration tutorial](collaboration.md).

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
"$LOCUST_BIN" blueprint example open >"$demo/open.json"
"$LOCUST_BIN" blueprint validate "$demo/open.json" >"$demo/inspection.json"
author blueprint draft create --id research --expected-revision 0 - <"$demo/open.json" >"$demo/draft.json"
revision=$(pick blueprint_draft.revision <"$demo/draft.json")
source_hash=$(pick blueprint_draft.source_hash <"$demo/draft.json")
author blueprint publish --draft research --id research-v1 \
  --expected-revision "$revision" --expected-source-hash "$source_hash" >"$demo/publication.json"
# A stale expected revision must not replace the saved source.
if author blueprint draft update --id research --expected-revision 0 '{}' >"$demo/conflict.json"; then
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
draft=read('draft.json')['result']['blueprint_draft']
publication=read('publication.json')['result']['blueprint_publication']
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

A source update uses `blueprint draft update --expected-revision REVISION`; a
presentation update has its own revision and does not change the semantic hash.
A CLI conflict returns `error.details` with the current record. Preserve the local
edit, reread the source and reconcile before retrying. Never silently overwrite
it or publish using a hash from an earlier draft. Refresh `blueprint drafts` or `blueprint draft show` to retrieve complete records
with their source revisions/hashes; refresh presentation separately. After
reconnect, repeat those reads and compare revisions before accepting cached
validation. Changing credentials invalidates the private catalog view. There is
no catalog notification subscription; goal-event cursors do not observe unbound
drafts.

To instantiate deliberately, use an enrolled agent allowed to manage goals and
pass the reviewed publication's `normalized_json` to `goal create --blueprint-json`.
Supply `--roles` and `--inputs` as JSON maps matching its declared slots. The
result pins the exact semantic definition and contextual bindings. Creating a
goal grants its creator local administration; contribution, review, flow and
execution grants remain explicit. The private author credential has no session
or goal rights, and publication does not distribute a draft to goal members.

The [concepts](concepts.md) explain the intended distinction between a definition,
an instance and a participant. The [implementation plan](../organization-blueprints-implementation-plan.md)
records the remaining runtime work.

## Correct diagnostics and test a custom pattern

Keep the exact diagnostic phase and JSON path when revising an invalid draft.
Syntax/type failures belong to shape checks; missing role references or unsupported
rule combinations require semantic correction. Unsupported versions require a
current-format definition, never conversion or an old-reader fallback. Contextual
binding/input failures need the actual instance and cannot be fixed by pretending
an offline report observed a local permission.

Combine only schema-supported constructs. Explain the result and compare its
normalized semantic identity before publishing it for review. Do not encode prose
prompts, drawings or labels as a hidden executable rule. Exclusive reservations,
mutable votes, vetoes and unimplemented capabilities cannot be introduced by a
custom field. The validator must refuse unsupported behavior explicitly.

Definition tests cover supported shape/semantics and expected explanation. Runtime
conformance additionally needs signed traces with missing proof, changed rules,
competing candidates, authority forks, duplicate delivery and restart. A preset
validation pass is not one of those runtime transcripts.

## Author with your agent

Describe the intended participants, allowed work, exact contribution evidence,
completion rule and whether a unique selection is required. Ask the agent to read
the inventory, current schema, operations and matching example first. Have it
report unsupported requirements rather than invent fields. Draft, validate,
explain and compare normalized semantics before requesting publication or binding.
Publication and goal creation remain distinct operations with distinct credentials
and grants.

The authoring discovery contract is public JSON and raw Markdown. Agents need
only the relevant article/schema/example, not the entire manual or a generated
mega-prompt. Preserve diagnostic codes and paths in a failed draft review; do not
hide errors with a permissive parser or local permission change.

## Use the editor on locust.farm

The editor at [locust.farm/blueprints](https://locust.farm/blueprints) builds a
blueprint without writing JSON. Choose one of six ways of working, change who
adds tasks, who works on them, when a result counts and whether one result is
picked, and add steps if Locust should add tasks in order. The page runs the same offline checks as
`locust blueprint validate`, shows Locust's explanation, and copies one prompt.

The prompt has your agent find Locust, check the schema version, save the
blueprint to a file and check its size and SHA-256, validate and explain it with
Locust, save a private draft and its name, and ask before publishing. It never
starts a goal. Its exact text is in the
[blueprint prompt contract](../blueprint-prompt.md). The editor keeps work in the
browser only; nothing reaches Locust until you paste the prompt.
