# Production T2 client qualification

Status: local actual-client investigation, with separate scripted-provider and
real-model records. No independent collaborators, physical network, installed
release, default interactive approval or worker confinement is established by
these runs. The [managed contract](../docs/managed-clients.md) describes the
implemented behavior; the [release ledger](../docs/release-evidence.md) retains
the remaining gates.

## Experiments

The [production harness](../scripts/check_t2_clients.py) uses isolated actual
clients, a scripted loopback provider, production SQLite/Iroh daemon state,
enrolled non-owner credentials and the actual `locust mcp`. A transparent
[observer](../scripts/client_qualification/observe_mcp.py) records correlated
JSON-RPC metadata. Actual native client shell tools execute an authored synthetic
workspace driver; this deliberately is not real-model planning or skill use.

The separate [real-model harness](../scripts/check_t2_models.py) invokes real
providers, installed skill files, native file/shell tools and the same production
daemon. The harness creates the synthetic base and assignment. Models inspect and
claim the task, materialize, change/test, submit, review, accept and explicitly
apply. The harness independently checks task/event/manifest IDs, file contents,
tests and unrelated dirty work. Acceptance is checked before a separate apply
turn, while source files are still unchanged.

The [managed harness](../scripts/check_managed_clients.py) invokes `locust client
run` through actual clients with a scripted provider. It observes durable
readiness and binding, claim generation, local interruption, client exit,
cancellation pending after daemon restart, explicit native resume and explicit
cancellation acknowledgment. Unit/CLI fixtures test failure windows separately;
they are not substituted for actual-client results.

All client profiles and daemon homes are disposable private directories. Raw
evidence stays ignored and private; reviewed results are retained here. Provider
authentication is the caller's selected runtime credential, never copied from an
existing interactive client login. One principal acts in both roles through
separate local sessions, so these runs do not establish two-person or
independent-account collaboration.

## Findings requiring separate claims

- Droid's native `Execute` returned `SIGKILL` even for `/bin/echo` under the
  scripted experiment's external-network guard. Allowing local bind/listen as
  well as local outbound traffic did not change that observation. The cause has
  not been established. MCP claim/progress/interruption/resume are separate from
  the failed native workspace execution. Real-provider runs, which need external
  networking, use a different profile and do not inherit this isolation claim.
- OpenAI rejected the production MCP schemas forwarded by Droid because their
  `u64::MAX` maximum annotations exceeded the provider's accepted numeric range.
  [The schema fix](../crates/locust/src/mcp/schema.rs) omits unsafe numeric upper
  annotations without lowering the typed Rust API range or inventing a new task
  limit. The regression checks provider-facing schema numbers and full-width API
  deserialization. Original failures and subsequent artifact identities must
  remain distinct in retained evidence.
- A native continuation failure is not repaired by relabeling a fresh session as
  a resume. Workflow completion through an explicitly new session, when used,
  is recorded separately from native-session continuity.
- Codex 0.153.4 exits with status 1 after a controlled SIGINT during an outstanding
  wait. Its [pinned exec source](https://github.com/openai/codex/blob/rust-v0.153.4/codex-rs/exec/src/lib.rs)
  explicitly maps an interrupted turn to that exit status. The harness accepts
  it only for this exact client/version after its own requested interruption;
  signals such as SIGKILL/SIGSEGV and other nonzero Codex exits still fail. Droid
  0.218.1 has a separate exact exception for exit 1 with the measured default-policy
  denial of `locust_note_add`; arbitrary denial-associated exits still fail.
  Natural child/bridge cleanup and a fresh resumed operation remain separate checks.

## Recorded results

The [scripted production record](evidence/t2-production-clients-2026-10-03.json)
retains exact binary/client hashes, task and contribution IDs, native sessions,
policy modes and individual assertions. The [real-model record](t2-real-model-qualification.md)
retains four mixed-client pairs and earlier failed attempts separately.

| Client | Scripted production MCP and workspace | Real-model coordinator and worker | Managed scripted lifecycle and recovery |
|---|---|---|---|
| Codex 0.153.4 | Task/contribution/apply, interruption, restart and resume passed; default write denied | Both roles and native coordinator resume passed | Thirteen normal checks and five recovery checks passed |
| Claude Code 2.1.280 | Task/contribution/apply, interruption, restart and resume passed; default headless read/write denied | Both roles and native coordinator resume passed | Thirteen normal checks and five recovery checks passed |
| Factory Droid 0.218.1 | MCP task/lifecycle passed; guarded native workspace execution failed; default write denied | Both roles completed; native coordinator resume failed, explicit fresh-session application passed | Thirteen normal checks and five recovery checks passed with scripted provider/backend |
| Pi 1.0.1 | Task/contribution/apply, interruption, restart and exact nested-path resume passed | Both roles and exact coordinator resume passed after path correction | Thirteen normal and four recovery checks passed; default blocking not run because native policy permits the tested operation |

The [managed record](evidence/managed-client-qualification-2026-10-03.json)
retains final normal and deliberate crash campaigns against the same local debug
artifact: `locust 0.1.0 (040da1187719-dirty) api 1 protocol 1`, SHA-256
`205169864dc78dca8d7c51c484b7d44bafab1cb69b619816bc577344b5c38c51`.
Its 172 recorded source hashes match the final verified source. Earlier
campaigns remain attributed to their separate artifacts. All final normal runs
observed natural cleanup. Crash tests deliberately killed only the owned
launcher, observed `Unknown` without spawn/signals and refused a duplicate
launch. Subsequent forced cleanup was needed for Codex, Claude and Droid; Pi's
observed processes exited after the deliberate fault. These are fault-scenario
cleanup observations, not normal-exit claims. The normal test
resumes after daemon restart, observes durable cancellation through ordinary
tools and explicitly acknowledges it. A client exit never supplies that outcome.

A production session/pending snapshot race initially made the launcher report an
error after a successful Codex cancellation acknowledgment. Those API reads are
separate: an intervening claim/outcome can make them inconsistent. The launcher
now retries that local observation before final exit, with a deterministic
regression and corrected four-client normal campaign. Earlier failures remain
attributable to their original artifacts.

Independent final review also corrected repeated local interrupt forwarding and
exit observation when a descendant retains output pipes, with deterministic
regressions and the final four-client campaigns. Harness JSON-RPC correlation and
denial-exit predicates were strengthened and successfully re-evaluated against
all twelve recorded production streams, with separate replay provenance.

Final source verification passed formatting, strict whole-workspace Clippy and
all **511 Rust tests** (zero failures, nine explicit ignores). The complete
Python suite passed **137 tests**. Documentation/link checks and exact selected-key
scans passed before committing. No ignored check is counted as a pass.

The real-model campaign included an observed provider-key disclosure through a
native tool result. Retained files were scrubbed and capture redaction was tested;
the [incident record](t2-real-model-qualification.md) explains why this does not
provide native-tool secrecy or worker confinement. No secrets are retained in
these reviewed JSON appendices. Automatic skill discovery, default interactive
approval, independent collaborators/accounts, physical networking, qualified
active hooks, automatic wake, installed packaging and confinement remain unrun.
Passing the measured subset does not close the remaining release gates.
