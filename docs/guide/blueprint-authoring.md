# Blueprint authoring

**Status: development authoring contract; organization execution is proposed.**
The first authoring slice provides offline definition checks. It is not a daemon
workflow for publishing, binding or running an organization.

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
```

`validate`, `explain` and `normalize` also accept `-` for JSON on standard input.
Schema, example and normalization commands emit raw JSON by default. `--json`
uses the CLI's structured result envelope. Inspection reports include validity,
diagnostics, normalized definition, semantic hash and effective-rule explanation.
Each diagnostic identifies a code, phase, path, message and suggested correction.

The example names are `open`, `coordinator`, `peer-review`, `independent-attempts`,
`review-panel` and `pipeline`. Source tests and export parity checks qualify the offline surface. They do not
qualify organization execution. The [generated reference](schema-reference.md)
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
| Contextual readiness | Proposed instance and action checks | Currently unavailable in this authoring slice |

A structurally valid definition is not proof of unique decision authority or
runtime readiness. A successful offline explanation does not publish a definition,
create an organization instance, grant access or execute an assignment.

## Review before runtime use

Review the effective rules alongside the draft. Keep the checked file and its
source/schema identity together. Publishing, binding and runtime action readiness
remain future work; this development manual provides no executable command for
those operations.

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
Those runtime steps remain separate until their actual operations exist.

The authoring discovery contract is public JSON and raw Markdown. Agents need
only the relevant article/schema/example, not the entire manual or a generated
mega-prompt. Preserve diagnostic codes and paths in a failed draft review; do not
hide errors with a permissive parser or local permission change.
