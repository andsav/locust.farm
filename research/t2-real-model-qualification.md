# T2 real-model client qualification

Date: October 3, 2026 (local host date). **Status: measured local real-model
capability evidence; native Droid continuation remains a failure. This is not
independent-participant or release support.**

The [real-model harness](../scripts/check_t2_models.py) extends the production
daemon/MCP checks with actual provider calls and model-authored native tool
work. Its [private-profile helper](../scripts/client_qualification/real_models.py)
uses exactly the selected ambient `OPENAI_API_KEY` or `ANTHROPIC_API_KEY`. It
copies no normal client profile or account store and supplies no Factory key or
fake Factory backend. Provider metadata confirmed the exact selected models:
`gpt-5.5` and `claude-sonnet-4-6`.

## Measured scope

Each pair uses one private production daemon and enrolled non-owner principal,
with two different protected session instances. The harness creates a two-file
synthetic Git repository, previews and exports its committed input, proposes and
assigns a task, and locally authorizes that exact assignment. These setup actions
are harness-authored, not model accomplishments.

The worker model must read the installed [Locust skill](../skills/locust/SKILL.md),
inspect actual MCP status/goal/task/pending responses, claim the exact assignment,
record progress, materialize the input with the trusted CLI, inspect both files,
fix `add(a, b)` from subtraction to addition, and execute the five supplied Python
unit tests. It captures only `calculator.py`, reviews the authenticated patch and
submits the exact base/patch/head using its returned claim generation. The harness
provides no script that performs these target operations for the model.

A separate coordinator coding client reads the skill and exact result event,
reviews the authenticated CLI diff, checks the original source/HEAD/local work,
and accepts the exact contribution. The harness independently verifies accepted
head with null integration and unchanged source before the application turn.
The coordinator applies through the trusted CLI and executes the tests again.
Independent file, test, task, event and workspace-binding reads check the result.
The original Git HEAD, test file and unrelated untracked file must survive.

These are explicitly permissive local experiments: Codex uses `-a never -s
danger-full-access`, Claude uses `--bare --dangerously-skip-permissions`, and
Droid uses `--skip-permissions-unsafe`. Pi retains its native policy without a
permission extension. The per-turn/cleanup budget is an operator-selected 180
seconds, not a product execution cap. The production daemon has a loopback/Unix
network guard; real-model clients require external provider networking.

## Artifacts and observations

The original immutable local debug binary reports `locust 0.1.0
(1481d7f4a0ee) api 1 protocol 1`; its SHA-256 is
`6283fd5f6f5359718a737f589822b9e56f2169028c826477ae93b9a0351c9ff0`.
The numeric-schema remediation uses a different immutable local debug binary,
reporting `locust 0.1.0 (e87f2af80b27-dirty) api 1 protocol 1`, SHA-256
`8903e2c8b1fc1705482f79721090c9f57513ad2b7236a0b826d691dfed4f7c6c`.
That second artifact includes in-progress managed-module source, but these runs
do not exercise managed launch. The original config-probe SHA-256 is
`fc0b585e183b03c4e30502703c50f75481a7f95130e09ddc06f8f83116aea4eb`;
the schema-fixed config-probe SHA-256 is
`edfd9fc8f686f655a7c42c2a2d5145039a807c257f77287606f4595399e406e7`.
These are local debug artifacts, not signed or installed release artifacts.

Installed clients measured here: Codex 0.153.4, Claude Code 2.1.280, Factory Droid
0.218.1, and Pi 1.0.1. Node 22.22.0 is available in the isolated Pi PATH.

| Coordinator / worker | Provider models | Observation |
|---|---|---|
| Codex / Claude | OpenAI `gpt-5.5` / Anthropic `claude-sonnet-4-6` | Full worker/review/accept/apply workflow and exact coordinator native resume passed on the original artifact |
| Claude / Droid | Anthropic `claude-sonnet-4-6` / OpenAI `gpt-5.5` | Full workflow and exact Claude coordinator resume passed on the schema-fixed artifact |
| Droid / Pi | OpenAI `gpt-5.5` / OpenAI `gpt-5.5` | Full worker/review/accept/apply workflow passed through an explicitly fresh coordinator native session on the schema-fixed artifact; attempted native continuation remains FAIL |
| Pi / Codex | OpenAI `gpt-5.5` / OpenAI `gpt-5.5` | Full workflow passed on the schema-fixed artifact with corrected nested native session path, exact coordinator ID and extended original history |

The Anthropic Droid fallback was separately selected and reported. Droid worker /
Claude coordinator passed the complete workflow on the original artifact with
both clients using `claude-sonnet-4-6`. Droid coordinator / Pi worker reached
acceptance, then exhibited the same empty-output native continuation failure.
A successful fallback does not erase the failed OpenAI path or establish native
Factory account authentication.

Local private reports retain invocation, binary/source digests, matched MCP
receipts, completed native tool inputs/results, protected-session instance
identifiers, daemon result identities and independent verification. Their
current locations are `output/t2-real-model-first/`,
`output/t2-real-model-claude-droid-schema-fixed/`,
`output/t2-real-model-droid-pi-fresh-scoped/`,
`output/t2-real-model-pi-codex-native-path-fixed/`, and the explicitly named failed
attempt and fallback directories. Runtime profiles are removed after each pair. This
document preserves the reviewed findings without runtime identity or capability
bytes; raw prompts and reports are private disposable evidence, not published
source material. The [reviewed compact evidence](evidence/t2-real-model-qualification-2026-10-03.json)
retains the selected pair checks, exact artifact/client digests, public object and
session identifiers, failed attempts, cleanup and incident metadata without raw
prompts, native output, private profile paths or secret capability bytes.

## Defects and evidence remediation

### OpenAI function-schema rejection

With production MCP configured, Droid BYOK on OpenAI returned HTTP 400 before any
tool invocation: `Invalid function parameters for 'tools[25].parameters': a
numeric value in the function parameters is too large.` The same key/model
passed a minimal readiness call without MCP, so readiness alone did not establish
production tool support.

The [MCP schema generator](../crates/locust/src/mcp/schema.rs) originally emitted
`u64::MAX` numeric maxima. The remediation omits maxima that cannot be represented
as safe JSON integers while retaining legitimate smaller limits and the typed
Rust API's `u64` validation. Droid's real OpenAI worker workflow passed after the
artifact change. The original rejected reports remain failures.

### Pi native session path

Pi 1.0.1's installed `migrateSessionsFromAgentRoot()` moves `*.jsonl` files
directly under its agent directory into `sessions/<encoded-cwd>` at startup.
The first real-model harness placed its explicit session there, so the next
invocation migrated it and created a new session at the old path. This was a
harness path error. A scripted default-then-lifecycle sequence can mask it if
the migration destination is already occupied.

The real-provider helper now selects
`.pi/agent/sessions/qualification-session.jsonl`. Explicit continuation requires
the same native header identifier and a history file that extends its original
bytes. Passing `--session` with the same path alone is insufficient evidence.
Earlier changed-identifier attempts remain failed resume observations.

### Native Droid continuation

Fresh Droid BYOK sessions successfully read the skill and use production MCP,
native `Read`/`Execute`/`ApplyPatch`, and the trusted workspace CLI without a
`FACTORY_API_KEY`. Continuing the coordinator with its returned native
`--session-id` exited 1 in approximately 1.6 seconds with empty stdout/stderr
under both OpenAI and Anthropic BYOK. The application turn did not execute.
This measures provider-key-only continuation failure; the empty output does not
identify its underlying cause. No existing Factory authentication was copied,
and no dummy Factory token or scripted backend was introduced into these runs.

The explicitly authorized follow-up started a fresh Droid native coordinator
session with the same scoped Locust protected session, repeated all required
identifiers and reconciled current daemon state. It applied the already reviewed
accepted contribution and ran the five tests. Independent final file/HEAD/WIP and
daemon checks passed. Its new native identifier differed from the original
coordinator's identifier, so the harness labels `fresh_coordinator_application`
PASS while keeping `explicit_coordinator_resume` FAIL. The aggregate client
execution assertion and process exit remain failed because the actual failed
continuation was not erased; all four observed turns nevertheless had natural
verified process cleanup. This is a successful fresh-session application path,
not native resume qualification.

### Private evidence disclosure

An early Pi application prompt referred to its prior CLI prefix without repeating
the explicit scoped paths. After the unintended new session, the model searched
for those paths and ran `ps eww`. That native tool exposed the selected OpenAI
provider key in a model-visible tool result and three private ignored evidence
files. No key value was included in chat or tracked research. The exact key bytes
were scrubbed from all three retained files, and subsequent scans found no key
bytes in the inspected evidence directories or tracked/non-ignored source files.

Application prompts now repeat the entire scoped CLI prefix and forbid process
environment inspection, credential searches, and secret reads. A tested capture
wrapper replaces explicitly named provider-key values before writing retained
stdout/stderr evidence. This is evidence hygiene: it does **not** redact native
tool results before the client sends them to its model provider, protect the
client's private session state, or establish secret containment under permissive
native execution. The earlier disclosure remains an observed limitation.

## Verification boundary

The [profile tests](../scripts/tests/test_real_model_profiles.py) and
[workflow evidence tests](../scripts/tests/test_t2_model_workflow.py) currently
pass 14 focused tests. They cover selected-key isolation, safe metadata/config,
separate policy modes, private skill placement, completed native-read evidence,
structured model completion, exact local source/HEAD/WIP boundaries, and capture
redaction including split stdout/stderr writes.
The final combined-tree Python repository suite passed 137 tests, including the
separate managed-client harness regressions. Documentation and local links are
checked against the staged files before committing this record.

Model completion and generated prose alone never pass the workflow predicates.
Matched successful production MCP responses, actual native tool receipts and
independent daemon/file/test observations are required. Prompts supplied the
installed skill path and required a completed native manifest read. This proves
explicit skill use; automatic discovery from the client's skill catalog was not
measured. Source skill placement alone is only preparation.

The [release evidence ledger](../docs/release-evidence.md) remains the authority
for broader gates. These local same-principal pairs do not establish independent
people/provider accounts, separate collaborating identities or machines, peer
transport, default interactive approval, worker confinement, managed launch,
signed packaging, installation or release support.
