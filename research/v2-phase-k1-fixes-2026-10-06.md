# Locust v2 phase K1 review fixes

Status: implementation and verification record, 6 October 2026.

This follows the [K1 review](v2-phase-k1-review-2026-10-06.md), re-reading its
findings after Phase 5 (the roles plan's Phase 4) landed in `8c086c1`, with
model baseline metadata in `dcb6952`. The owner explicitly waived the initial
clean-tree prerequisite. Other sessions' changes are outside this change,
including the shared checkout's later `46cc3b3` plan correction and the existing
root README edit. No protected plan or companion is edited here.

## Findings and disposition

The paths below are the review's headings; their old line numbers are retained
only as identifiers. Current source links and test names give the evidence.

| Review heading | Disposition and evidence |
| --- | --- |
| `goal/chain.rs:451`: governance fork cost | **Left: design and plan ownership.** The replay rule is unchanged. Corrected the overly broad test comment: only a member whose own log has no record anchored in the cut suffix can continue. Plan corrections are listed below. |
| `node/flow.rs:49`: an unsignable automatic step stops the daemon | **Fixed.** Failed effects are skipped for the rest of that pass, exposed as `cannot_materialize`, and retried after a later change or startup. Store failures still fence the daemon. `an_unsignable_step_stalls_without_failing_open_join_or_receive` exercises an oversized header, another successful stage, restart, `Host::join` and `Replica::receive`. |
| `requests/goals.rs:270`: hostile host names a remote agent as its own | **Left as requested.** This requires a host outside the owner's trusted-host design. No `goal leave` change. |
| `cli/invitations.rs:233`: ticket signature overclaim | **Fixed.** The review says only “Signature: verified.” It retains the separate statement that authority and admission are confirmed while joining. |
| `node/access.rs:258`: rules refusal exposes the wrong host | **Already fixed by Phase 5.** The current code uses `state().host` and `host_name`. Added assertions that `Why::Rules.host` equals the goal's host and differs from its governance key in `replay_refusal_precedes_local_level_and_owner_act_persists`. |
| `requests/daemon.rs:144`: reconnect misses deliveries | **Fixed.** `land` refreshes deliveries after committed local identity changes are absorbed, before driving automatic steps. The regression checks immediate receipt without any new signed step, and persistence through restart. |
| `cli/workspace.rs:301`: agent is told the host is disconnected | **Fixed.** `workspace init` refuses a caller without `--owner` before looking up the host's agent. A real Node/CLI test uses a second enrolled member and confirms both agents remain connected. Removed the superseded non-owner epoch path. |
| `formation-editor/ui/problems.ts:77`: wrong diagnostic sentence | **Fixed.** The editor explains that the host's computer adds a stage task and that its work/completion rules must name members or a role. An editor test checks the rendered sentence from a real inspection. |
| `goal/flow.rs:266`: revised stage gets no usable offers | **Fixed.** Offer templates resolve `current_round` at their signing anchor. `a_signed_automatic_offer_and_its_acceptance_follow_each_stage_round` signs and accepts offers before and after revision, forward, reversed and reloaded. No new revised-round limitation remains. |
| `goal/fold.rs:313`: replay accepts impossible stage revision rules | **Fixed.** Replay reuses the stage creator check on the resolved work and completion rules. Tests reject independent creator starts, offered-to-creator starts and nested creator completion rules in all three replay orders. |
| `requests/tasks.rs:127`: revision drops the task type | **Fixed.** An omitted type retains the current round's type. The daemon test confirms a safe type survives and an explicit unusable type is refused without a stored record. |
| `cli/mod.rs:591`: disconnected-agent wording and missing recovery command | **Fixed.** Status, named/default local agent selection, goal add, daemon credential and owner-on-behalf refusals, doctor, onboarding conflict and command help point to `agent reconnect`. Exact-key commands continue directly to daemon authorization. Revoke/reconnect remain usable for an already disconnected identity. |
| `goal/flow.rs:40`: stage runner read from history | **Fixed.** Stage and stage-review runners read `chain.state.governance`. |
| `node/definitions.rs:162`: held invalid rules appear missing | **Fixed.** The daemon strictly decodes, normalizes and verifies the referenced hash without discarding semantic-invalid formations. Replay then excludes them as `InvalidDefinition`, preserving the earlier rules. Normalization preserves invalid key strings for validation instead of panicking. A two-Node test receives the rules record and then its encrypted definition, and verifies exclusion after arrival and restart. It covers both a stage creator violation and an invalid participant key. |
| `requests/goals.rs:356`: bad formation is refused after confirmation | **Fixed.** CLI create/bind inspect before constructing the plan, and bind inspects its final composed rules too. CLI and daemon errors include diagnostic code, message and correction. A CLI test verifies no plan or daemon request is emitted for the invalid stage formation. |
| `cli/presentation.rs:559`: stage task prints the key in effective rules | **Fixed.** Human rules output omits the creator field already described by the “Created by” line. Typed JSON retains its existing fields. The no-key rendering regression now includes an actual creator value. |
| `cli/presentation.rs:616`: first-record body prints the key | **Fixed.** Human first-record output describes creation, the host's agent and the definition. The rendering regression now includes a genesis body. |
| `proto/api.rs:1241`: two stale fingerprint comments | **Fixed.** Updated both `Response::Joined.governance` and `Invitation.governance`; regenerated the runtime contract. |
| `goal/tests.rs:1913`: replay order and signed offer gaps | **Fixed.** Governance fork, host-agent review fork and stage creator tests run through forward/reverse/reload comparisons. Added a signed offer and acceptance in each stage round, and a final-position fork between two stage steps. The latter agrees on `Pending(ForkProof)` for those ordinary steps, no materialized effects and halted governance. |
| `node/replica_tests.rs:239`: no real daemon `hosted_here=false` | **Fixed.** Host and member Nodes assert true/false respectively. A separate production CLI over a member Node's Unix socket renders “Host: on another computer” after a signed join and replicated records/content. |
| `requests/daemon.rs:145`: reconnect never proves a waiting step runs | **Fixed.** A regression commits a review trigger while its author is disconnected, observes `RunnerRevoked`, reconnects it and checks the materialized signatures. |
| `requests/levels.rs:118`: governance stalled branch untested | **Fixed.** The stage fork delivery test checks hosted governance steps report `Halted`, while the member's daemon reports no locally stalled governance work. The oversized-step test covers `CannotMaterialize`. |
| `node/tests/authorization.rs:195`, `:643`, `:695`: disconnected host-agent command gaps | **Fixed.** `disconnecting_the_hosts_agent_stops_no_host_command` runs rules bind, task revise, member removal, workspace epoch, invitation creation/revocation and farm on/off with the host's agent disconnected, checking signed records still belong to the goal's key. |
| `node/tests/farm.rs:406`: publication exemption test is too weak | **Fixed.** The test now includes an effective stage step signed by the governance key before publication, plus the excluded forged consent. Removing the exemption is checked as a deliberate mutation below. |
| `organization.cases.json:265`: weak shared stage cases and reconnect no-op | **Fixed.** Added untyped-stage rules, offered-to-creator, nested `all`/`any`, decisions-only and work-only type cases, regenerated vectors, and added a connected-agent CLI reconnect that changes nothing. |
| `Organization.tla:253`: no host-step kind | **Left under the small-change exception.** The current model requires every ancestor in the governance log to anchor to its predecessor, and all modeled non-governance work goes through member authorization. Representing mixed governance/host-step ancestry correctly requires changing both predicates and adding matched safety, mutation and reachability cases. This change instead records the boundary below and adds Rust replay cases. |
| `node/authoring.rs:208`: a halt is called missing history | **Fixed.** Governance `next_place` returns `Halted` for an actual halt. The gap case still returns `Unavailable` and becomes signable after its predecessor arrives. Existing delivery regressions cover both. |
| `cli/presentation.rs:151`: agent sees “Host: you” | **Fixed.** An agent sees “Host: your owner” locally; an owner sees “Host: you”. Rendering tests and a real Node/CLI test cover the distinction. |
| `docs/guide/help.md:31`: guide's host definition | **Left as requested.** The phase rewriting the guides owns it. |
| `docs/master-plan.md:3`: stale phase status | **Left: plan ownership.** Requested corrections are recorded below; no plan or companion was edited. |

## Implementation and test references

- Automatic signing, delivery and reconnect: [node flow](../crates/locust-core/src/node/flow.rs), [commit](../crates/locust-core/src/node/commit.rs), [delivery tests](../crates/locust-core/src/node/tests/delivery.rs), [authorization tests](../crates/locust-core/src/node/tests/authorization.rs).
- Stage rounds and creator eligibility: [effect evaluation](../crates/locust-core/src/goal/flow.rs), [replay](../crates/locust-core/src/goal/fold.rs), [shared validation](../crates/locust-core/src/organization/validation.rs), [replay tests](../crates/locust-core/src/goal/tests.rs).
- Held definitions: [definition lookup](../crates/locust-core/src/node/definitions.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs).
- Human output and preflight: [presentation](../crates/locust/src/cli/presentation.rs), [owner commands](../crates/locust/src/cli/only_you.rs), [CLI tests](../crates/locust/tests/cli.rs), [real Node/CLI tests](../crates/locust/tests/t2_flow.rs).
- Publication: [farm tests](../crates/locust-core/src/node/tests/farm.rs).
- Editor mirror: [diagnostic rendering](../sites/locust.farm/src/lib/formation-editor/ui/problems.ts), [its test](../sites/locust.farm/src/lib/formation-editor/ui/problems.test.ts), [shared cases](../docs/reference/conformance/organization.cases.json).
- Current behavior is documented in [formations](../docs/formations.md); the [runtime contract](../docs/reference/generated/runtime.contract.json) exposes the new stall reason.

## Follow-ups for later phases and the plan owners

A fork in the governance log stops more than host commands. Any member whose own
log contains a record anchored on a cut governance record remains pending through
that record's descendants, even if later descendants anchor on the surviving
head. The daemon can still sign those records, but they cannot count. A member
with no such ancestor can continue under surviving rules. State this cost beside
the single-key choice, the K1 risk note and the G2 halt wording; do not promise
that every member's work counts in a halted goal. Changing that replay rule or
splitting automatic steps into another log is a design change, left for its owner.

The master, host-safety, roles and joinable-farms plans and their companions should
name K1 as built (`0a4bbc2`, `cb1acaa`, build notes `a6664a1`) and the names/roles
phase as built (`3a56930`, `8c086c1`, metadata `dcb6952`). Statements that K1 is
still future work and old current-protocol/API numbers need the plan owners'
review. This is a request to reconcile current plan text, not a change to a
historical snapshot description. The other session's `46cc3b3` already corrects
parts of the host-safety plan and is preserved.

The [Organization model](tla/Organization.tla) still does not model host steps,
a fork between a step and a governance record, or a governance record following a
step. Its governance-key invariant is valid for its modeled kinds only. Rust
replay tests here exercise signed automatic offers, acceptances and step forks,
but do not establish formal refinement or replace a future model extension.
No model source or cases are changed by this task, so the conditional model gate
is not applicable.

The existing limit on automatic offers for subtasks is unchanged. Revised stage
rounds now have their own automatic offers; do not document them as blocked.
Failed materialization is local status, retried at the next goal change or
startup. It is not a new replicated event or permission to bypass store failures.

## Verification

The final source and generated exports passed these gates. Python commands used
`/opt/homebrew/bin/python3`, as requested.

| Gate | Result |
| --- | --- |
| `cargo fmt --all --check` | Passed. |
| `cargo clippy --locked --workspace --all-targets -- -D warnings` | Passed. |
| `RUST_TEST_THREADS=1 cargo test --locked --workspace --no-fail-fast` | Passed: 1,103 tests passed, none failed, 14 ignored across the test binaries and doc tests. |
| `cargo build --locked -p locust` | Passed; the following export and recipe checks use this rebuilt binary. |
| `python3 scripts/check_formations.py` | Passed: six examples validate, exports match and shared vectors cover every diagnostic code. |
| `python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60` | Passed: `shared-workspace-loop`, `local-collaboration`, `private-authoring`, `separate-goal-export`. |
| `python3 -m unittest discover -s scripts/tests` | Passed: 300 tests run, three optional signing tests skipped because their Python signing dependencies were unavailable. |
| `python3 scripts/check_docs.py` | Passed after staging the report and its index entry. |
| Site: `npm run lint`, `npm run check`, `npm test`, `npm run build` | All passed: no Svelte errors or warnings, 219 tests passed, 89 prerendered routes checked. |
| `python3 scripts/check_tla.py --suite organization` | Not applicable: no model or model cases changed. |

The publication mutation was also executed: removing the governance exemption in
`node/farm.rs` made
`publication_needs_no_consent_from_the_governance_key` fail (`Suspend` instead of
`Upload`). The exemption was restored before the passing full Rust suite; the
production publication source is unchanged.

The 14 ignored Rust cases are the suite's opt-in network, installed-client,
performance, simulation and store-interposer checks, including subprocess helper
entry points. They are not claimed as additional passes. The daemon regressions
exercise the production Node engine with local fixture stores; the rendering
integration tests run the production CLI against it through a Unix socket. These
are automated local results, not a two-computer or live-agent qualification.
Intermediate failing fixtures were corrected before the final gates; no initial
failed run is claimed as a pass.
