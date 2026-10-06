# Locust v2 phase R4 build notes: names and roles in the goal

Status: implemented, 6 October 2026; source, model, site and recipe checks
pass, with local network qualification waived by the owner. This
note covers only **Phase 4: Names and roles in the goal** in the
[roles plan](../docs/roles-and-permissions-plan.md) and its
[companion](../docs/roles-and-permissions-plan-details.md). Phase 5 is not
part of this work. The starting point was `a6664a1`, after the
[K1 build](v2-phase-k1-build-notes-2026-10-06.md) and the
[phases 1–3 build](v2-phases-1-3-build-notes-2026-10-06.md). The protected
plan files are owned by another session and were not edited for this phase.

The model and evidence are committed as `3a56930`; the implementation,
tests, recipes, contracts and site changes as `8c086c1`. A following
metadata-only commit pins the 42 Organization cases to that implementation.
All 100 retained source hashes match its committed tree.

## Implemented behavior and its checks

### Signed names and roles

- [event.rs](../crates/locust-proto/src/event.rs) adds a signed member name
  and optional role to admissions, removes holders from `RulesBinding`,
  and adds `RoleHolders` at discriminant 25. The protocol and API remain 7.
  There is no old-byte reader, migration, compatibility flag or alternate
  runtime. [vectors.rs](../crates/locust-proto/src/vectors.rs) retains the
  updated signed vectors.
- [invite.rs](../crates/locust-proto/src/invite.rs) signs the host agent's
  name and optional role into a ticket and the joining member's name into
  the join request. The node persists that information through retries,
  uses the requested name in the admission, and reports the ticket's host
  name while the founding transcript is still arriving.
- [chain.rs](../crates/locust-core/src/goal/chain.rs) keeps role holders in
  each governance snapshot. A newly declared role starts with the host's
  agent; removing its final holder restores the host's agent. Roles remain
  available for work under earlier rules. Role updates require a nonempty,
  ascending, distinct list of admitted members.
- [goals.rs](../crates/locust-core/src/node/requests/goals.rs) implements
  host-only `role.give` and `role.take`, with expected-holder comparison.
  A deciding role has one holder; a group role adds members. A deciding
  role cannot be taken from the host's agent. Taking a group role from its
  sole host holder is a no-op. Missing historical definitions block role
  changes and rules changes whose role kinds cannot be checked. A role's
  deciding/group kind is enforced by the owner API across every binding.
- [roles.rs](../crates/locust/src/cli/roles.rs) applies role commands at
  once, prints duties from the formation, and gives an inverse using
  signed identities and shell-quoted role names. It distinguishes a full
  Undo from the final-holder fallback's `Give it back:`. Member resolution
  accepts signed names and unique key prefixes, and refuses ambiguity.
- [node role tests](../crates/locust-core/src/node/tests/roles.rs) cover
  expected-holder conflicts, fallback, no-op, earlier roles, kind changes,
  unavailable definitions, signed invitation names/roles and remote keys.
  [CLI tests](../crates/locust/tests/cli.rs) exercise the resulting commands,
  printed inverses, names, plans, automatic reviewers and rules changes.

### Anchored rules and counting

- [fold.rs](../crates/locust-core/src/goal/fold.rs) and
  [rules.rs](../crates/locust-core/src/goal/rules.rs) resolve role holders
  and only-member eligibility at the record's governance anchor. A task
  keeps its pinned rules but sees the role holders at each later act.
  [delegation.rs](../crates/locust-core/src/goal/delegation.rs) compares
  selector meaning symbolically when checking narrowing, so today's
  coincident holders cannot make unrelated roles interchangeable.
- A member's latest effective review or named-check attestation controls
  current completion. Selection evidence already pinned by a decision
  stays valid. A role change does not erase reviews signed while eligible.
  [views.rs](../crates/locust-core/src/node/views.rs) uses the same latest
  review lookup for pending counts, with opinion reviews excluded.
- A review under rules with no review requirement is an opinion. It is
  recorded, but neither completes the result nor satisfies a stage's
  review prerequisite. New reviewers receive review requests for results
  still needing review; already approved or selected work gets none.
- [presets.rs](../crates/locust-proto/src/organization/presets.rs) exposes
  the six formations in plan order, with `directed` replacing
  `coordinator`. The default is `peer-review`. Peer review and pipeline
  allow their only member's contribution to count; this is read at that
  contribution's anchor, so adding another member does not undo it.
  Review-panel needs two other reviewers and gives joining members the
  reviewer role unless the host says otherwise.
- [goal tests](../crates/locust-core/src/goal/tests.rs),
  [workspace replay tests](../crates/locust-core/src/goal/workspace_tests.rs),
  [organization tests](../crates/locust-core/src/organization/tests.rs) and
  the node role tests exercise past-role authority, latest verdicts,
  selection pins, only-member changes, symbolic narrowing, opinions,
  pending work, restart and arrivals in different orders.

### Shared files and local authority

- The host's first source-free files in an empty workspace epoch count
  without review. Later proposals follow the pinned completion rule.
  Agent calls cannot perform the owner's first capture, including through
  replacement of an already registered local folder.
- `workspace init` pins the host agent as integrator and the goal's rule
  as completion; `--integrator` is removed. `rules bind` carries an enabled
  workspace into the new rules in the same durable transaction. Its new
  epoch references the new binding and checkpoints the accepted revision,
  or remains unseeded when no revision is accepted. Signing uses adjacent
  governance positions; replay remains the sole writer of goal rules.
- [workspace lifecycle tests](../crates/locust-core/src/node/tests/workspace_lifecycle.rs)
  cover the two-record transaction, carried files, obsolete proposals,
  approval of the next change, restart, and two encoded local replicas.
  [workspace request tests](../crates/locust-core/src/node/tests/workspace.rs)
  cover owner-only first capture and replacement.
- Agent signing still follows `sign_for`: replay the candidate first,
  then apply the local level, and pass that checked state to commit. Names
  also populate the existing structured refusal fields. No Phase 5 view
  or refusal redesign is included.

### Recipes, contracts and editor

Executable recipes and affected harnesses use current names, roles and
workspace behavior. Generated contracts, frozen formation vectors and the
six [formation examples](../examples/formations) were regenerated through
`check_formations.py --write`. The site's formation editor understands the
only-member selector, mirrors Rust validation and descriptions, defaults
to Peer review, and offers Directed with lead/reviewer roles. Its existing
generic canonical encoding already supports the new selector tag.

## Model first and retained evidence

The [Organization model](tla/Organization.tla) now snapshots role holders
and reads them at an act's anchor. Seven new cases cover role changes,
lead changes and final-holder removal, with three safety runs, three
reachability witnesses and a mutation that ignores the role anchor. All
seven matched their expected outcome **before any Rust or site code edit**.
The gate is retained in
[phase4-role-gate.json](evidence/tla/organization/phase4-role-gate.json).

The first complete 42-case run matched every semantic expectation but
exited 1 because implementation edits changed its watched source snapshot
after the seven-case gate. This is retained, with that limitation, in
[phase4-model-first.json](evidence/tla/organization/phase4-model-first.json).
A second full run uses frozen completed source; its result is recorded in
the [evidence index](evidence/tla/organization/README.md).

The completed implementation commit cannot include its own hash. Therefore
the initial registry names the actual K1 baseline plus the Phase 4 model
amendment, and a following metadata-only commit pins Organization cases to
the implementation commit. Earlier result JSON remains immutable, with
the baseline and source digests actually checked. Session/effects baseline
metadata is unchanged.

## Departures and readings of the plan

1. The model keeps existing event IDs, introduces role records 20/21, and
   advances later governance positions by two instead of renumbering the
   old scenario records. An initial development attempt placed the rules
   record at sequence 10 instead of 8, making role actions unreachable;
   the mutation caught this and it was corrected before the passing gate.
2. The companion's review-panel test name says "until one more" while the
   completion rule needs two approvals excluding the author. The
   implementation and test require two other reviewers, matching the
   rule and terminal text.
3. For a custom compound completion rule that names multiple qualifying
   group review roles, automatic admission chooses the lexically first
   role. The plan describes a singular counting role without a tie-break.
   Deciding roles are never assigned through an invitation.
4. Role refusals keep the daemon's `ApiError.message` generic and put the
   role name in structured details. This follows the earlier refusal
   constraint against inserting untrusted member/role text directly into
   an error sentence. CLI output sanitizes names and quotes role arguments.
5. Chronological ordering after a lead change also applies when checking
   an observed closure's ancestry, not only in the final projection named
   by the plan. Otherwise different holders' unrelated sequence numbers
   could choose an obsolete close/reopen decision.
6. Opinion handling also excludes opinions from stage review prerequisites,
   and review-request creation also skips other results on a task that
   already has a selection. These complete the plan's counting and pending
   invariants in paths beyond its enumerated edits.
7. The editor's existing generic normalization handles `only_member`, so
   it needs round-trip tests rather than a redundant encoding arm. When a
   completion rule also requires a named check, the only-member branch
   wraps the approval component; the check still has to pass. Displayed
   parentheses preserve that meaning.
8. The browser fixtures were updated beyond the plan's listed files to
   follow the changed defaults and roles. Existing tests that require
   author declaration now explicitly select `open`; they do not rely on
   the changed default. Current guide prose made false by this phase was
   repaired beside its recipes: preset names/order, only-member behavior
   and latest reviews. The obsolete claim that no signed replay test
   covers compound/check rules was removed.
9. The implementation uses test modules and helpers already present after
   K1 rather than recreating the plan's older locations and exact names.
   Symbolic narrowing lives beside the implementation in `delegation.rs`.
   The once-per-empty-epoch test checks carried-file refusal and restored
   empty acceptance; competing first acceptances are covered by the
   existing workspace competing-successor tests. There is no separate
   transferred-closer regression beyond the new decision chronology tests
   and existing observed-closure tests. Boxing the enlarged network send
   command and a test-only sync input enum keeps Clippy's size check
   satisfied without changing wire encoding.
10. The implementation/baseline commit split and the first full model
    run's source-change result are recorded above rather than presenting
    the earlier full run as a clean completed-source gate.
11. Internal rules resolution accepts a role map and an `only_member`
    value derived from the same snapshot, rather than accepting a whole
    membership map. `Goal::latest_reviews` exposes the verifier's shared
    lookup to node views. The wire fixture's final `RoleHolders` uses
    `reviewer` rather than the companion's `lead`; its index is still 25.
12. P4's terminal examples are not copied byte for byte. Create keeps the
    existing rules sentence followed by a separate reviewers/missing line;
    the invite plan keeps relative expiry and invitation counts. Existing
    Phase 2/3 identity, level and sharing context remains. Local status adds
    the signed host name after `Host: you`. The reviewer holders/duties and
    safe inverse commands follow P4's structure.
13. A rules-change plan pins the workspace epoch and policy, but not an
    incidental head revision or pending-proposal count; active work may
    proceed while the person reads the plan. An incoming explicit workspace
    policy is preserved; a missing one gets host-agent integration and the
    new goal completion rule. `workspace init` always uses the host agent
    while preserving an explicit workspace completion rule.
14. The goal-add retry digest includes the chosen invitation role. A changed
    role for the same member/day/head must not reuse an older invitation's
    retry key. This is required by the new role-bearing admission but was
    not enumerated in the plan's edits.
15. The local T1 harness now waits for the exact offer in `pending.ask_first`
    before allowing and starting it. Its previous "receives offer" wait
    tested only whether the task appeared on the board; a run reached the
    task before its offer and correctly got a conflict when starting.

## Verification

Checks use `/opt/homebrew/bin/python3` (3.12 or newer), not the system Python.

- `cargo fmt --all --check` and
  `cargo clippy --locked --workspace --all-targets -- -D warnings`: pass.
- `RUST_TEST_THREADS=1 cargo test --locked --workspace --no-fail-fast`:
  **1,090 passed, 0 failed, 14 ignored**. The ignored cases are the existing
  two production idle-interval transport tests, schema-cost measurement,
  two installed-client parser probes, two long simulations, two performance
  measurements, two discovery probes, two subprocess helpers and the
  macOS store-flush interposer test. No ignore was added. This final run
  includes the extended maximum-host-name invitation test and the added
  oversized-role test; the focused 17 invitation tests also pass.
- `/opt/homebrew/bin/python3 scripts/check_tla.py --suite organization`:
  **42 expected outcomes, exit 0, no timeouts**, with
  `source_changed_during_run: false`. Run
  `20261006T203500Z-6f425b43` is retained as
  [phase4.json](evidence/tla/organization/phase4.json), in model commit
  `3a56930`. The checker itself was not changed.
- `scripts/check_formations.py`: exports verified, six examples validate,
  every diagnostic code covered by the conformance vectors.
- `scripts/check_documentation.py --binary target/debug/locust --timeout 60`:
  all four recipes pass (`shared-workspace-loop`, `local-collaboration`,
  `private-authoring`, `separate-goal-export`).
- `python3 -m unittest discover -s scripts/tests`: 300 tests, OK, three
  skipped because this Python lacks the optional `blake3`/`cryptography`
  farm-signing dependencies. The fixture-compilation failure line in this
  suite is an expected negative fixture, not a suite failure.
- `scripts/check_docs.py`: pass with the new note/index and new linked
  sources staged.
- In `sites/locust.farm`, all four required commands pass on the final
  guide text: lint; check with 0 errors and 0 warnings; 213 tests; build
  with 89 routes, 14 raw articles and 10 exact assets checked.
- The 18 formation-editor Playwright tests also pass, including phone
  width, changed preset/role words, sharing and real CLI validation.
- A fresh-home real CLI walkthrough passes 26 assertions across 74 calls.
  It verifies solo/default approval, old-result stability after admission,
  later approval/rejection, review-panel defaults and `--no-role`, named
  role changes, lead fallback/refusal and earlier-role lifetime. Thirteen
  `event show` readbacks confirm effective host-signed admissions and role
  updates. The local artifact is `output/r4-walkthrough.json`; its own
  loopback daemon was stopped and reaped.
- The requested source searches find no `binding.roles`, quoted
  `coordinator`/`judge`, `--integrator`, or `administrator` in their scoped
  Rust/site paths. The custom demo's own coordinator role is still a valid
  custom role and uses give/take rather than the removed flag.

Local network qualification is blocked by a reproduced pre-existing
key-only discovery failure. With the required absolute binary path and
`--network local`, T1 reaches admission, work, review and selection on all
three daemons, then times out exchanging notes after the host stops and a
non-host restarts. Two independent runs reproduce it. Their traces contain
host-to-peer connections but no connection between the two non-host peers.
Both peers sign and read their own notes. The local mode disables relays
and Mainline, so those peers need mDNS discovery rather than the ticket's
host contact hints.

The isolated ignored test
`tests::mdns_finds_a_peer_by_key_without_contact_hints` was explicitly run
and fails with `transport: Connect` in about 10 seconds. The same test was
compiled and run from a clean export of `b45d149` (K1 Rust code, before
Phase 4; later commits at that point contain only model/plan documents),
and fails identically. This narrows the blocker to the existing discovery
path or this Mac's network environment, not the new role/admission logic;
it does not by itself prove which OS setting or transport detail is at
fault. No network permission or production timeout was changed.

The failed T1 reports are under
`output/t1-local/20261006T204823Z-5655eb52` and
`output/t1-local/20261006T205025Z-9548d42e`; isolated current/baseline probe
logs are `output/r4-mdns.log` and `output/r4-mdns-k1.log`.
`simulate_machines/run.py --quick` also timed out in the two-machine
catch-up and three-machine offline-exchange cases. After the owner replied
"I can't check right now, we can skip this one" to the local-network
qualification question, the remaining crash scenario was stopped and its
owned daemons were reaped. The runner exited 143 after its cleanup handler;
the quick suite is **not a pass**. This user-approved waiver applies to
the unresolved local-network qualification, not to the source/model/site
gates listed above.
The partial quick-run report is
`output/sim/20261006T205220Z-2f62/results.json`.

### Failures resolved during verification

The first parallel Rust run failed six process tests: daemon reconciliation,
two managed-client tests and three MCP pipe/startup tests. Reconciliation
passed alone; all six passed in the serial full run. The explicit-signals
test passed. No production delay, timeout or retry was changed to make
these tests pass.

Development compile/lint runs found enlarged-enum warnings and a missing
`Path` qualification in the final printed-Undo test; these were corrected.
An earlier Python run exposed the old coordinator role in the workspace
qualification helper and a concurrently replaced debug binary; the helper
now uses reviewer, and the stable-binary run passes. A first T1 invocation
used a relative binary path, which its parser refuses; subsequent runs use
the required absolute path. The task-before-offer race and the model's
initial sequence/source-change failures are recorded above.

The evidence is bounded to source tests, bounded formal models, local
processes/replicas and the local site/browser. It does not claim a public
deployment, two physical machines or a real-model collaboration run.
