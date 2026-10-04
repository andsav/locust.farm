# Last-mile implementation plan

Date: 2026-10-04. **Status: implementation plan; work packages remain proposed except for the partial W3 implementation noted below.** The owner requested this plan and the amendments to its source research. That does not select the remaining distribution, permission, protocol, publication or paid-run decisions. Apache-2.0 was separately selected and is already present.

This plan sequences work from the revised [last-mile research](../research/last-mile-experience.md). It supplements the [main implementation plan](implementation-plan.md), [protocol-1 contract](protocol-v1.md), [installation contract](installation.md) and [coding workflow](t2-workflow.md). Their implemented authorization and recovery rules remain authoritative until a specific replacement is accepted and tested. All command names and data shapes introduced below are proposed interfaces, not commands to run today.

**Implementation update, 2026-10-04:** W3's bound launcher, installed-skill prefix, `up` and `agent add` enrollment orchestration are implemented with component recovery tests; see [installation](installation.md) and [resumable onboarding](onboarding.md). Fresh Codex and Claude Code profiles previously passed the [native launcher workflow](../research/bound-cli-launcher-qualification.md) with scripted providers. Native installed qualification of the new onboarding route is pending. Separate display metadata/renaming, atomic managed-session client metadata and `doctor` integration remain deferred; real-model selection and interactive approval remain unverified.

## 1. Outcome and scope

The first product outcome is one person using Claude Code and Codex on one Mac to complete a small real change without relaying messages between them. The agents must use separate enrolled principals and sessions. One agent discovers information relevant to the other; the other reads it while it can still affect the work. The resulting patch is reviewed, accepted, explicitly applied to the intended checkout and verified there. An accepted event alone is insufficient.

The next outcome repeats that journey for two people on two physical machines, with inspectable invitations, meaningful names, local authorization and useful communication. Public visualization follows a successful private workflow. Linux and the other baseline clients retain separate qualification; a Mac pass does not qualify them.

Onboarding aspirations are at most three terminal commands, no hand-copied identifiers and under ten minutes on a fresh Mac with both clients already installed and authenticated. Record actual elapsed time, approval prompts, manual interventions, identifier copying and points of confusion. Command count must not hide a long wizard, and a missed time target must not weaken the review/application criterion.

The first release does not require open-task arbitration, coordinator handoff, a hosted service, a browser peer, per-goal reputation, automated stop hooks, a new graphical application or a new execution framework. Each has a disposition below. No estimate of “an afternoon” or a promised calendar date substitutes for acceptance evidence.

## 2. Baseline and implementation seams

Source review baseline: `1a4f9ff`. Historical measurements remain those in the [research evidence](../research/evidence/last-mile-experience-2026-10-04.json); they were not rerun for this plan. Current source gives these concrete starting points:

| Area | Existing seam | Consequence for the work |
|---|---|---|
| Local API and operation policy | [api.rs](../crates/locust-proto/src/api.rs), [version constants](../crates/locust-proto/src/lib.rs) | API 1 has typed requests, authorization categories and tool flags. Wire-shape changes need an explicit API revision, not just a CLI edit |
| Notes and task reads | [notes.rs](../crates/locust-core/src/node/requests/notes.rs), [reading.rs](../crates/locust-core/src/node/requests/reading.rs), [views.rs](../crates/locust-core/src/node/views.rs) | Notes currently collect all matching text. Filtering after that read does not protect the daemon |
| Feed and acknowledgment | [feed.rs](../crates/locust-core/src/node/feed.rs), [reading.rs](../crates/locust-core/src/node/requests/reading.rs) | Feed order is local arrival order and persisted. An explicit `events.after` writes the principal/owner cursor; viewers do not. It must not be reused as an invisible acknowledgment side effect |
| Grants, claims and acting as a principal | [access.rs](../crates/locust-core/src/node/access.rs), [claims.rs](../crates/locust-core/src/node/requests/claims.rs), [API](../crates/locust-proto/src/api.rs) | Owner `on_behalf` exists. Execute, decide and takeover are distinct. `GoalGrants` has no expiry field |
| Session records | [sessions.rs](../crates/locust-core/src/node/requests/sessions.rs), [adapter delivery](../crates/locust-adapter/src/delivery.rs) | `session.report` replaces a record without compare-and-swap. An MCP initializer must not race a managed runner and overwrite its state |
| MCP and skill | [mcp.rs](../crates/locust/src/mcp.rs), [schema.rs](../crates/locust/src/mcp/schema.rs), [skill](../skills/locust/SKILL.md) | One-to-one operation tools, duplicated text/structured serialization and a workspace-heavy skill are current behavior |
| Setup and service | [setup.rs](../crates/locust/src/installation/setup.rs), [service ownership](../crates/locust/src/installation/service_install.rs), [installation.rs](../crates/locust/src/installation.rs) | Setup fingerprints existing credential/session files and journals owned edits. `up` needs staged recovery around these operations |
| Persistent formats | [record encoding](../crates/locust-core/src/node/records.rs), [SQLite schema](../crates/locust-store/src/schema.rs) | SQLite has a schema version and transactional migrations; individual postcard local records have no version envelope. These are separate compatibility concerns |
| Invitations | [invite.rs](../crates/locust-proto/src/invite.rs), [invitation requests](../crates/locust-core/src/node/requests/invitations.rs) | A ticket is a secret single-use capability. Inspection can be local; new signed metadata/admission semantics need a compatibility design |
| Release and site | [packaging](packaging.md), [release ledger](release-evidence.md), [builder](../scripts/build_release.py), [guide](../sites/locust.farm/src/lib/onboarding/guide.ts) | Licence selected; package inclusion, public origin, production trust and public artifact verification remain work. The guide has no setup artifact |

Before each package, refresh Git status and these seams. Preserve unrelated work. Keep the existing crate boundaries initially; put shared CLI/MCP presentation in a module of `locust`. Extract a library only when a second actual consumer, such as Polaris or the exporter, requires it. Never duplicate the authoritative event fold in a renderer.

## 3. Constraints and decision register

The following constraints retain existing behavior unless explicitly identified as a proposed convention:

- Typed transitions, membership, coordinator authority, claim generation and local grants govern execution and finalization. Notes cannot override them. Enforcement lives in [core request handling](../crates/locust-core/src/node/requests/mod.rs) and its [authorization tests](../crates/locust-core/src/node/tests/authorization.rs).
- Setup does not grant work/sharing permission or alter client approval policy. Existing grant authorization persists. A bound launcher is convenience and attribution, not isolation from a process that can read the owner's credential; see [installation](installation.md) and [main plan](implementation-plan.md).
- Goal selection stays explicit for writes. Short names are human display/selection conveniences; MCP writes ultimately target a uniquely resolved typed identifier.
- Transport/framing limits stay explicit. Pagination and preview controls may bound a read; they do not introduce hidden task runtime, model-token, author-content or execution caps. Preserve full retrievable content within existing declared protocol limits.
- Submission, acceptance, application, worker-reported checks, independent verification, local observation and remote claims remain distinguishable.
- Proposed presentation convention: contextual peer text is attributed data within the authorized task. Errors, news, hooks and notifications contain daemon-authored kinds/counts/identifiers. Terminal escaping and HTML text rendering preserve the underlying signed text; neither is an injection-proof sandbox.
- No clock observation alone reassigns work or proves a process stopped. Older received content cannot be recalled.

Resolve decisions at their dependent work package, not as one questionnaire blocking all work:

| ID | Decision and proposed default | Needed before |
|---|---|---|
| D1 | Public origin, trust bootstrap, signing custody, and first qualified platform. Apache-2.0 is already chosen; recommend qualifying Mac first while stating Linux's actual status | W1 publication |
| D2 | Locust is independently usable without Polaris; recommend yes, keep Polaris out of the required newcomer route | W1 site copy |
| D3 | Offer goal-scoped execution permission separately from setup. Default to explicit per-assignment authorization unless the owner chooses standing permission. Decide expiry and its effect on existing claims; do not describe expiry as implemented | W4 permission UX; expiry requires W2 contract |
| D4 | Permit derived `brief`/`news` tool output and scoped peer text in task responses; recommend yes with W2/W5 retrieval and acknowledgment rules | W2/W5 |
| D5 | Keep invitation mint/redeem and owner administration out of model tools; recommend person-driven ticket handling. Select reviewed client-profile ownership/hooks and whether the owner credential must be isolated from the client sandbox; otherwise state the same-user limitation | W3/W7 tool/profile changes |
| D6 | Choose real-model clients/models, run count and maximum spend from current prices. No paid run is authorized by this plan | W6 real-model execution |
| D7 | Approve the API/storage upgrade strategy and later replicated changes. Recommend a small local API revision with protocol 1 unchanged first; retain data through migration. Fresh homes only by explicit owner choice | W2 implementation/upgrade |
| D8 | Remote display metadata defaults. Recommend per-goal coordinator labels; client/display-name sharing visibly selected at join, person/machine labels off | W7 |
| D9 | Decide whether bare/self-accepted reports merely get labels or can be refused, and whether coordinator corrections can block submit. Recommend labels only in the first releases | W5; optional W9 |
| D10 | Public consent and origin. Recommend each member's owner consenting to their records, separate third-party rendering origin, and no hosted publisher initially | W8 |
| D11 | Local graphical view, ring design, public levels and recorded demo content. Recommend a table-first recorded example; no liveness animation for snapshots | W8/W9 |

These consolidate the research's sixteen questions without silently accepting them. Open work, handoff and holder-status protocols require separate accepted designs if later selected. A coordinator-led group does not need competitive task claiming merely to justify the word “swarm.”

## 4. Work sequence and dependencies

| Package | Outcome | Dependencies | Contract scope |
|---|---|---|---|
| W0 | Scope, decisions and fixture/evidence design | None | Documentation and harness fixtures |
| W1 | Obtainable, truthfully described candidate | W0; D1/D2 before publication | Packaging/bootstrap/site |
| W2 | Reliable paginated reads and acknowledgment | W0, D4/D7 | Local API and local storage; no planned replicated change |
| W3 | Repeatable setup and bound clients | W0; W1 for installed qualification | CLI/setup; W2 only for new local metadata |
| W4 | A person can see and authorize work | W3; W2 for durable cross-goal views and expiry if chosen | CLI and local authorization |
| W5 | Agents exchange useful context in normal work | W2/W4, D4/D9 | MCP, skill, projections, existing report payload |
| W6 | A reviewed change is applied and verified | W1 through W5; D6 for paid runs | Harnesses and recorded evidence |
| W7 | Two people complete the same journey | W6; D5/D8 and scoped protocol decisions | Local API; replicated changes only where justified |
| W8 | Recorded/public view of proven collaboration | W6 recorded example; W7 and D10/D11 for external snapshots | Export schema and site; consent contract if required |
| W9 | Optional extensions | Evidence from prior packages and specific decisions | Feature-specific |

W1, the design portion of W2 and reversible W3 work can progress independently. This is a dependency map, not authorization to delegate agents. W6 fixture preparation starts in W0 and runs alongside implementation, not as a separate research phase before any product work. Publishing is not required to exercise local candidates, but the external-first-user claim requires public distribution proof.

### W0. Lock the first journey and its evidence

1. Record decisions already made and mark all others pending. Refresh the licence/distribution statements when W1 changes them; do not overwrite historical measurements.
2. Design a small synthetic repository with a meaningful check and two related tasks. One task reveals a constraint that changes the other task's correct implementation. The final checkout also contains unrelated work whose preservation is asserted.
3. Define owner, coordinator, worker, viewer and session identities in the fixture. Do not reuse the prior harness's same-principal roles as independent-member evidence.
4. Specify the observation format: source/binary/harness hashes, client/model versions, prompt hashes, policy/profile mode, task and event identifiers, findings read/used, final patch identity, application result and independent checks. Keep provider payloads, credentials, tickets and unrelated files out of retained output.
5. Add the scenario to the existing [real-model harness](../scripts/check_t2_models.py) and [client qualification helpers](../scripts/client_qualification/real_models.py) when execution support is needed; avoid a second harness for the same lifecycle.

**Done when:** the fixture has an objective correct result and a failure when the cross-task constraint is ignored; its scripted control can distinguish note-written, note-delivered, note-used, accepted and applied. No model-behavior conclusion follows from that control.

### W1. Distribution, trust and truthful entry points

Code areas: [release builder](../scripts/build_release.py), [package verification](../crates/locust/src/package.rs), [installation](../crates/locust/src/installation.rs), [release workflow](../.github/workflows/release-build.yml), [site routes](../sites/locust.farm/src/routes/+page.svelte), [onboarding guide](../sites/locust.farm/src/lib/onboarding/guide.ts).

1. Include `LICENSE` in candidate bundles and their integrity coverage. The current verifier declares exactly two payload paths; update builder, manifest/verifier contract and installation/removal tests together. Do not just add an unsigned incidental file.
2. Prepare a platform-selected bootstrap that obtains the identified candidate, signed manifest and withdrawal policy from D1's origin. Document how the first verifier/trust root is obtained; downloading a verifier beside an untrusted candidate is not independent authenticity. Show the exact trust source and first-install assumption.
3. Keep existing plan/apply validation, withdrawal monotonicity, source/architecture checks, staging and conservative removal. Add explicit network failure/retry and candidate-withdrawal behavior. Never fall back to an unsigned executable.
4. Implement the D7 cross-version installer path before publishing a changed API candidate. The present equality check prevents an old installer from installing a new API/protocol version. W2's upgrade sequence must be exercised using the public installation path.
5. Wire the guide's artifact only after an independently fetched published artifact has been verified. Replace stale unavailability/licence text; remove Polaris as a required step. Add concise concepts, person commands, agent tools and recovery pages generated or checked against actual supported operations.
6. Use a reviewed first-party recorded run only after W6; until then describe current readiness factually. Show each platform's actual availability, including any incomplete Linux qualification.

**Tests/evidence:** build/verifier tests cover the added payload, tampered/missing licence, bad signature, wrong architecture, withdrawn candidate, interrupted install and conservative removal. Native Mac install and public re-download verify exact artifact hashes, trust and source commit. Linux needs its own native artifact/install result; existing emulated results remain separately labelled. Site gates and live TLS/route/artifact checks are required for deployment claims.

**Done when:** a newcomer obtains the correct licensed candidate from the selected origin and reaches `up` without manually constructing signing inputs. Unknown origin/custody blocks publication, not preparation of a reviewable local candidate.

### W2. Local read contract, acknowledgment and upgrade

Code areas: [API](../crates/locust-proto/src/api.rs), [request dispatch](../crates/locust-core/src/node/requests/mod.rs), [reading](../crates/locust-core/src/node/requests/reading.rs), [notes](../crates/locust-core/src/node/requests/notes.rs), [views](../crates/locust-core/src/node/views.rs), [feed](../crates/locust-core/src/node/feed.rs), [local records](../crates/locust-core/src/node/records.rs), [store migrations](../crates/locust-store/src/schema.rs).

**W2.1 — Freeze a small contract before wiring callers.** Define typed operations for note pages, a task thread, a goal snapshot, explicit acknowledgment and read-only identifier resolution. Names are provisional until this contract is reviewed. Update operation authorization, request/response matching, serialization fixtures and version reporting together. Keep the replicated event format and protocol-1 fold unchanged unless a separately selected requirement cannot fit.

Proposed semantics:

| Read/operation | Required properties |
|---|---|
| Note page | Explicit goal and optional subject, page/preview controls, stable event IDs, author, supersession, original length, availability, continuation and observed revision |
| Task thread | Task text/reference, assignment/attempt history, progress, applicable notes, result/verdict and reason; paginated, attributed and full-readable |
| Goal snapshot | One coherent local revision/decision head for members, board and pending sections. Local peer/session observations carry their own observation revision/time if outside that fold |
| Feed page | Position, event kind, subject/task/attempt and preview/reference; fetching does not acknowledge |
| Acknowledgment | Session-bound, goal-bound, explicit delivered content/stream position; cannot acknowledge another session or undisplayed gaps |
| Resolve | Read-only, typed namespace and explicit goal; unique prefix only; normal visibility rules prevent cross-goal enumeration |
| Recorded write | Event ID plus the revision of that commit when needed; historical replay of an idempotent result does not fabricate a new revision |

**W2.2 — Separate three concepts.** A continuation means where to fetch next; a wait revision means which state was observed; an acknowledgment means which content the client explicitly reports consuming. None proves that the model understood the content. Notifications never clear pending work, which remains derived from authoritative task state.

A single high-water mark cannot acknowledge a filtered task preview that omitted earlier relevant notes. Use per-subscription/task positions or explicit delivered event acknowledgments, with a contiguous watermark only where there are no unacknowledged gaps. Choose and document one representation before implementation. It must support late materialization of unavailable payloads and retain unread status until actual retrieval/acknowledgment. Validate acknowledgment against caller/session/goal and available delivered positions; reject future or foreign positions. The owner's view and each viewer must not advance an agent's cursor.

A restarted bridge reads persisted acknowledgment. A new session starts with current actionable state and a labelled history policy; it does not inherit another session's read status silently. Handle revocation and session closure through the same authorization as ordinary reads. Own writes may suppress self-notifications but never skip peer content. X5's consecutive-revision optimization is optional and cannot replace this rule.

**W2.3 — Bound retrieval before constructing output.** Select page entries before decrypting/loading text. A large note yields a declared preview and a complete retrieval path within transport framing constraints. Report superseded, withdrawn and unavailable content accurately. Do not silently hide history. Reader-selected filtering/muting must not suppress authoritative cancellations, current verdicts or halted state.

Use a coherent view in one request. For paginated mutable sections, return a view token/revision and an explicit stale-view response if consistency cannot be maintained; do not combine pages from different revisions and call them one snapshot. An append-only feed may page to a fixed captured upper position. Do not implement an unbounded automatic retry loop to manufacture a stable snapshot under constant writes.

**W2.4 — Protect attention at the same time.** Add per-author presentation quotas, visible overflow, progress coalescing and local mute to the selection layer. Full authorized records remain retrievable. Preserve original Unicode/content; escape for each renderer. No new replicated write quotas are implied.

**W2.5 — Make format upgrades recoverable.** Distinguish the local API version, SQLite schema version, local record encoding version, replicated protocol and future export schema. Prefer new tagged/versioned local records plus an explicit conversion for affected old records. Test restart midway through conversion and refuse newer unsupported storage before writes. Preserve identity, signing continuity, grants, task history and workspace bindings. Do not copy an active signer into two usable homes. Backup/restore must obey the existing signing-watermark recovery contract. Old binaries must not write upgraded formats; software rollback is not automatically a safe data rollback. If migration cannot be supported, document a non-destructive read/export route and obtain explicit fresh-home approval before any reset.

**Tests:** two sessions sharing a principal have independent acknowledgment; viewer/person reads change neither; paging/preview does not clear omitted text; peer writes interleaved with own writes remain unread; lost responses and duplicate writes are safe; restart preserves receipts; filtered streams, late payloads, supersession and revocation behave correctly; large notes and a flooding author do not force full-history responses. Snapshot races produce explicit staleness. Migration tests use old-format fixtures and interrupted upgrade paths.

**Done when:** CLI and MCP can consume bounded, coherent task context without hidden cursor mutation or loss across restart. Protocol-1 replicas still exchange unchanged events. The native installed upgrade passes, not merely fresh-home unit tests.

### W3. `up`, local identities and the bound CLI

Code areas: [CLI setup](../crates/locust/src/cli/setup.rs), [setup implementation/tests](../crates/locust/src/installation/setup.rs), [service ownership](../crates/locust/src/installation/service_install.rs), [client config](../crates/locust-adapter/src/config.rs), [doctor](../crates/locust/src/cli/doctor.rs), [session requests](../crates/locust-core/src/node/requests/sessions.rs).

1. Add `locust up` and `locust agent add <client>` as orchestration over existing service/enrollment/session/setup operations. Client detection presents candidates and actual profile paths; it is not permission to edit every detected profile.
2. Review service/profile/workspace selections, enroll a principal and create a protected session, then build and review the setup plan using those actual files. Persist a versioned private stage journal recording ownership and completed operations. Resume from checked state after failure; do not blindly rerun enrollment or grant permission as compensation.
3. Use client kind plus generated word as a default local display name, separate from the stable file-safe slug and principal key. Support renaming display metadata without moving credential files or changing identity. Person and machine labels remain empty unless explicitly supplied.
4. Write an owned launcher with absolute executable/home/credential/session references. Invoke without shell interpolation, do not log secret bytes, and reject caller overrides of the bound credential, session, home or owner/on-behalf authority. Point the installed skill to its actual launcher without mutating the signed source artifact; own and verify any generated wrapper skill as setup output.
5. Keep account login/provider credentials and client approval policies outside setup. Preserve unrelated configuration and reject modified owned entries/collisions as the existing journal does. Remove only unchanged owned launchers/configuration; preserve identity/data on uninstall.
6. Record normalized, self-reported client kind without overwriting a managed session's native ID, capabilities or lifecycle record. A `session.show` followed by `session.report` is racy: use a dedicated metadata operation or W2 compare-and-set/initialize-if-absent semantics, or defer N2 until that exists.
7. `doctor` reports one actionable next step per failed prerequisite: installation identity, service observation, API readiness, credential binding, skill/registration/launcher, peer reachability when relevant. Sandbox access to the owner credential is a qualified probe with a stated client/profile scope, not an inferred global guarantee.

**Tests:** interrupt after each stage, retry without duplicate principals, stale setup plan, client-owned edits, profile collision, missing executable, paths containing spaces, modified launcher, credential override attempts and managed-session metadata race. Test credential references without exposing their contents. Repeat setup/removal preserves unrelated configuration.

**Done when:** both selected clients can discover Locust and use native workspace commands via their launcher without a harness-written command prefix or searching process environments. Configuration-ready, daemon-ready and model-ready remain separate observations.

### W4. Human views, permissions and recovery

Code areas: [CLI arguments](../crates/locust/src/cli/args.rs), [CLI dispatch](../crates/locust/src/cli/mod.rs), [authorization](../crates/locust-core/src/node/access.rs), [daemon requests](../crates/locust-core/src/node/requests/daemon.rs), W2 reads.

1. Add human rendering for existing reads, `locust show`, `watch` and an owner `inbox`. Keep versioned machine-readable JSON and full identifiers there. Start with selected-goal views, then aggregate cross-goal actionable items without giving an agent the owner's credential. For the first goal, let the person select its local agents by name: owner orchestration can use existing invite/join operations internally, wait for actual admission and avoid hand-copied tickets. This is a separate reviewed goal action after setup, not automatic sharing during `up`; W7 can replace its internals with the dedicated local-admission operation.
2. Show names plus disambiguating short IDs; resolve against the typed visible namespace and reject ambiguous prefixes. Names/aliases resolve only for person-driven CLI selection. Labels are not identity proof, and an eight-hex display is not a security-strength fingerprint.
3. Give standing grants, member removal, sessions and viewer enrollment named commands. Reuse existing `on_behalf` for owner-driven assign/review/accept/reject operations with an explicit principal. Human control must not require an open coordinator model session.
4. Offer `allow <agent> --goal G` separately from setup and join. State whether it grants execute, decide or takeover; execute must not imply the other two. Provide inspect/revoke. If D3 chooses expiry, add persisted local policy, checks on use and sleep/wake/clock tests; specify existing-claim behavior and never claim expiry stops a process.
5. Render authorization-required, task rejected, cancellation requested, halted, disconnected and unavailable content with the actual next step. Model-facing responses contain no owner command. Person-facing commands use typed IDs, not executable peer text.
6. `watch` uses viewer/non-acknowledging reads and a caller-selected wait. Show last observed state and sync age; no moving indicator implies remote execution. Make exit/cancellation clean.
7. Add viewer create/list/revoke in a first principal-scoped implementation with explicit scope display. W7 adds goal/owner-read-only scopes. No loopback server is needed.
8. The review/apply path shows exact submitted artifact, worker-reported checks, acceptance and local application separately. Keep existing patch review/apply validation and preserve unrelated WIP. Full conflict dry-run UX may follow in W7, but W6 still requires a real review and successful explicit apply.

**Tests:** malicious titles cannot overwrite terminal lines or inject commands; duplicate names and ambiguous prefixes refuse selection; read views do not advance agent acknowledgment; permissions are per agent/goal; revocation/expiry and owner-on-behalf restrictions hold; agent tools cannot invoke owner actions; rejected tasks link to reasons; halt/offline/cancel-requested never render as completed or stopped.

**Done when:** the person can identify pending work, authorize it, recover from a closed coordinator session, inspect a result and apply it without copying long IDs or editing JSON. Permission friction is measured, not removed through hidden grants.

### W5. Agent context, useful findings and reports

Code areas: [MCP server](../crates/locust/src/mcp.rs), [tool schemas](../crates/locust/src/mcp/schema.rs), [skill](../skills/locust/SKILL.md), W2 read contracts and W4 presentation module.

1. Add derived `locust_brief` and `news` using W2 typed reads. Prioritize owned tasks, current instructions, attempts/verdicts, review work, accepted plan, then wider board/notes. Expose configurable previews and continuation; no invented token ceiling. A response labels unavailable or omitted sections rather than synthesizing missing context.
2. Put scoped task-thread previews on show, claim and progress responses. A successful write plus a failed supplemental context read must still report the committed write's identity; never tell the agent to retry a committed write merely because enrichment failed. Preserve exact idempotency semantics and classify any fresh derived context separately from the recorded write result.
3. Separate news from peer text. Counts and identifiers point to a read; retrieval and explicit acknowledgment clear unread state. Include cancellations/plan changes/verdicts. A goal-wide note is discoverable through brief without becoming an interrupt for every member.
4. Rewrite the skill and MCP instructions around the actual loop: brief, read task, claim, share useful discoveries when found, report progress, submit an evidence-bearing report, and distinguish submission from acceptance/application. After compaction, reconstruct from durable state. Coordinator notes clarify the authorized task; changes to scope or authority require the proper transition/owner permission.
5. Introduce a versioned report envelope in the existing result payload with summary, reported checks, remaining learnings or `none`, and references to earlier findings. Retain a plain-text fallback for historic results. Envelope validation is initially CLI/MCP behavior, not replicated enforcement against alternate clients. Do not require both a note and a duplicated report, or introduce a new report size ceiling. Mark checks as worker-reported and self-acceptance as a fact, without refusing it under an unselected policy.
6. Fix tool hints by operation semantics, not by “all writes are destructive.” Remove read-only idempotency-key schema repetition, explain meaningful parameters, default empty arrays in the adapter, and preserve typed validation. Account for legacy event-read cursor mutation while transitioning. Tool hints never grant permission.
7. Distinguish unclaimed/stale-generation/takeover errors and include field/expected identifier form or actual limit. Show a current generation only where authorized; never advise a stale client to reuse it without a valid claim. Preserve existing error codes or explicitly version changes.
8. Keep text and structured result representations by default. The [MCP specification](https://modelcontextprotocol.io/specification/2025-06-18/server/tools#structured-content) recommends both for compatibility. Measure each client's model-visible representation before any client-specific optimization. Wire byte counts alone do not prove duplicate token consumption.
9. Reduce tool count only after common-path observations support a specific consolidation. Removing member removal, withdrawal, takeover or invitations from MCP changes capabilities even if the wire API stays stable: update skill/schema/qualification together and follow D5. Add X6 opt-in local unknown-tool/argument-name diagnostics only after the main path; never record argument values.

**Tests:** brief at fresh session/resume/compaction, thread on successful writes, write committed but enrichment unavailable, duplicate-write replay, omitted-content acknowledgment, no peer text in news/errors, legacy reports, empty checks, malformed envelope, Unicode preservation and terminal escaping, context flood, cancellation interleaved with progress, and ordinary read/claim/submit permission policies.

**Done when:** deterministic fixtures show the context arrives on the expected path and no source text or permission state is lost. Useful model behavior remains unverified until W6.

### W6. Qualify the first useful collaboration

Extend [installed-client checks](../scripts/check_installed_clients.py), [real-model checks](../scripts/check_t2_models.py), [operational checks](../scripts/check_operations.py) and their [tests](../scripts/tests/test_t2_model_workflow.py). Reuse process ownership, redaction and exact-artifact binding.

1. Run the W0 fixture first through deterministic drivers to verify observation and failure classification. These runs test mechanics only.
2. With D6 authorization, run Claude Code and Codex through installed profiles with separate principals/sessions and the shipped skill. Give the goal and authorized scope, not a prescribed tool sequence, result text or hidden CLI prefix. Use a two-client case for the first journey and the research's three-agent dependency case for broader communication behavior.
3. Observe default interactive approvals separately from headless permissive harness settings. An unavailable account/client or blocked approval is recorded as such; do not silently switch flags and call it the same result.
4. Require a timely finding, actual recipient retrieval and a traceable implementation consequence before submission. Validate the artifact's correctness independently. Tool counts are diagnostic; repeated phrasing of a note is not proof it influenced the work. Without a paired control, claim demonstrated use in this fixture, not a measured productivity improvement.
5. Complete review, acceptance, explicit apply and post-apply checks. Assert the intended checkout and unrelated WIP. Exercise one rejection/revision, restart/resume and temporarily unavailable peer/content. Do not let the harness author the model's target operations and then credit the model.
6. Run synthetic adversarial cases: a note requests a canary file outside sharing scope; a member floods another task; names/notes contain escape sequences, misleading Unicode and role words. Retain safe event IDs/verdicts, not real secrets. Passing examples do not certify universal injection resistance.
7. Report installation time separately from model execution/review time, and record all manual interventions. If the ten-minute aspiration fails, identify the blocking interaction without relaxing the useful-change criterion.
8. Write retained evidence under `research/`, index it, and update the release ledger and implementation status. Raw disposable runs can live in `output/`; essential findings cannot exist only there.

**Done when:** the exact installed candidate completes the first journey, the participant can explain permission and result states, and the evidence distinguishes scripted, real-model, approval-policy, native-install and public-distribution proof. Failed/superseded attempts remain counted or explicitly excluded with reasons. A first external user then repeats the path; internal qualification alone is not external adoption.

### W7. Two people, names, invitations and scoped viewers

Code areas: [invitation format](../crates/locust-proto/src/invite.rs), [events](../crates/locust-proto/src/event.rs), [invitation handling](../crates/locust-core/src/node/requests/invitations.rs), [network](../crates/locust/src/daemon/network.rs), [authorization](../crates/locust-core/src/node/access.rs), [setup](../crates/locust/src/installation/setup.rs), [transport harness](../scripts/check_transport_probe.py).

**W7.1 — Names and tickets.** Implement local ticket inspection without redemption, explicit/default expiry with its value shown, inventory and revocation. Prefer protected file/stdin input to exposing a ticket in shell history; inspection never logs its capability. Show exactly which fields are signed/bound and which are unverified presentation. Bind any new goal title/inviter label to the authenticated invitation; a friendly name alone proves nothing.

Coordinator-assigned per-goal labels should be the first remote display feature. The existing optional admission payload may encode them without a new event kind, but define payload format, historic absence, old-reader behavior, confidentiality and any validation changes before claiming protocol compatibility. A new ticket format, profile event or pending-admission exchange needs its own version/qualification if selected. D8 determines which self-reported fields join shares; machine/person labels are not inferred from hostnames.

Implement a reviewed person-driven `up --join` flow that installs/setup-resumes, previews authenticated ticket facts and the private-only sharing policy (or a later implemented publication policy), accepts once, then shows joining/admitted/refused distinctly. Join never grants execution permission. Add an owner operation for admitting another local agent without manually passing a ticket, using the same authoritative membership transition. Removal from MCP alone is not a security boundary: enforce D5's chosen local authority at the daemon, splitting broad manage-goals authority if needed. Verify admission replay, expiry, revocation, first redemption and retry semantics.

**W7.2 — Read completeness.** Complete coordinator pending items: document proposals, reassignment, unassigned proposals and leave requests. Add owner read-only and goal-scoped viewer credentials; enforce scope at every read, blob/content fetch, watch and aggregate view. Define revocation for already-open connections. A goal-scoped viewer cannot enumerate another goal or advance agent acknowledgment.

Add local “heard here” time, progress count and sync observation without calling an agent running or stalled. Old records with unknown arrival time stay unknown; replay/import time is not historic activity. A remote holder-status feature remains deferred unless its separate protocol is selected. Add acceptance explanation if justified by review UX; determine its authenticated representation rather than treating it as an unsigned UI annotation.

**W7.3 — Recovery and clients.** Extend review/apply with a dry-run conflict list sharing the actual apply validator; revalidate at apply to handle races. Complete named session/leave/recovery views and read-only event access. Add the Droid setup target with its own owned-profile merge tests and client discovery qualification. Pi and Droid outcomes remain separate from Claude/Codex. Report any usage only as client-reported, per agent and with its measurement scope; distinguish absent from zero and do not total across people or publish it.

**Tests/evidence:** run offline/reordered admission and label propagation, label collisions/spoofing, ticket secrets absent from logs/model context, viewer scope/revocation, payload unavailability, note delivery after reconnect and preservation of result/apply semantics. Where replicated semantics change, update protocol docs/vectors/fold/sync tests and relevant formal-model mappings; do not cite protocol-0 results as protocol-2 proof. Repeat operational/install/client campaigns appropriate to the changed contract.

**Done when:** two independently operated physical machines complete review and application; the assignee uses a remote finding without human relay; permissions and invitation context are understood. Record direct/relay route and actual recovery observations separately. A same-host simulation cannot satisfy this gate.

### W8. Recorded and public visualization

Code areas: W2 snapshot and W5 projection modules, [site](../sites/locust.farm/src/routes/+page.svelte), [swarm renderer](../sites/locust.farm/src/lib/swarm/renderer.ts). Merak's layout is design input only until verified against real multi-machine data.

1. After W6, build a hand-reviewed first-party replay containing a real rejection and accepted/applied change. Capture board snapshots at known local revisions. Label them “as this daemon saw it”; the browser does not implement event folding. Static snapshots do not animate agent liveness.
2. Define a separate versioned public schema with an explicit allowlist. Counts, board and approved text are separate export levels. Use export-local opaque IDs; omit principal/endpoint IDs, source hashes, filesystem paths, grants, invitations, session details and other private structural fields.
3. Implement `export --public` as an owner-reviewed action. D10 must specify consent for each member's records, revocation before future exports and what may appear in totals. A local reviewed consent manifest may support a static prototype; any promise of shared immutable per-goal publication policy needs an authenticated protocol representation before shipping. Never infer consent from membership.
4. Explicitly select public titles/labels/text. A schema allowlist cannot prove arbitrary prose contains no path, credential or copied key. Add synthetic canary tests and a review preview; do not promise a test can recognize every secret. Prior published bytes cannot be recalled.
5. Render a table as primary content, with optional SVG map, accessible labels/keyboard navigation and text nodes. Use an independent origin for third-party snapshots; keep the install trust origin first-party. No raw HTML, automatic peer URLs or external fetches from snapshot text.
6. Validate a link-fragment checksum if used and label it “matches the link.” It proves byte consistency with that link, not identity, consent or protocol validity. Do not put private content in URL parameters or analytics.
7. Extract shared projection code only once both actual consumers need it. Keep private snapshot, agent brief and public schema separate types so export cannot accidentally serialize a private view wholesale.

**Tests/evidence:** schema allowlist, synthetic forbidden-field/canary fixtures, no-consent exclusion, malformed/future schema, checksum mismatch, HTML/script/control-character inputs, large-content continuation, keyboard/table access and screenshot review with actual captured data. Run site gates and live-verify the published replay/origin boundary.

**Done when:** a visitor can understand a real recorded collaboration without misleading liveness or private identifiers. Public user exports remain unavailable until consent and origin decisions are implemented and verified. Hosted near-live publishing is W9, not an implied part of this package.

### W9. Explicitly deferred extensions

| Feature | Trigger and design required before implementation | Acceptance evidence |
|---|---|---|
| Session-start/compaction hooks (K11) | Qualified client lifecycle and reviewed setup-owned entry; identifiers/counts only, no write or closed-client activation implied | Per-client hooks, profile ownership, restart/compaction, no unsolicited peer text |
| Submit correction gate (K12) | D9 selects a coordinator-only acknowledgment rule; define newer correction races, retry and stale/cancelled assignments | No other member can block submit; correction cannot expand permission; retries do not livelock |
| Open work (K14) | User need plus an accepted rule for concurrent offline takers and coordinator resolution | Deterministic convergence, fencing and no implicit exclusive execution claim |
| Coordinator handoff (K14) | Signed transfer authority, concurrent/offline successors, key distribution and abandoned coordinator recovery | Adversarial fork/replay/restart cases and updated formal correspondence |
| Review refusal/typed notes/closure (K8/K14) | Reader/user need for each semantic; define who sets policy, historical interpretation and author eligibility | Replay and policy enforcement on all replicas; no self-report promoted to verified fact |
| Replaceable holder status (K9/K14) | Define authority, freshness, disconnect and replay semantics independently of durable task state | Stale/reordered/offline records never imply running, takeover or completion |
| Local graphical page (V7) | D11 and demand beyond CLI; prefer a private self-contained file | File permissions, no accidental external fetches; authenticated Host-checked listener only if separately selected |
| Hosted publishing (V6) | Operator, retention, consent, expiry/unpublish/takedown, deployment and storage costs | End-to-end deletion/expiry/access tests; honest non-recallability after download |
| Merak worker integration (X7) | Separate Merak implementation request and typed Locust contract | Runner claims/settles, reads attributed brief, reports actual run record; native end-to-end evidence |
| Usage/desire-path refinements (H11/X6) | Actual client measurements and opt-in diagnostics need | Missing usage stays unknown; no global totals or recorded argument values/secrets |

No work package silently implements these to finish a diagram or share a protocol version bump.

## 5. Verification and evidence policy

Run repository-required gates on every completed logical implementation change before committing:

```sh
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

Use the pinned toolchain. Site-only changes instead run `npm run lint`, `npm run check`, `npm test` and `npm run build` in `sites/locust.farm/`. Changed Python helpers get their relevant unit tests; changes to the documentation checker require `python3 -m unittest discover -s scripts/tests`. Stage new docs/index entries and run `python3 scripts/check_docs.py`; documentation-only work does not need a Rust rebuild.

Beyond these gates, each package's scenario tests are required evidence for its behavior. Relevant existing harnesses are [installation](../scripts/check_installation.py), [installed clients](../scripts/check_installed_clients.py), [managed sessions](../scripts/check_managed_clients.py), [managed recovery](../scripts/check_managed_recovery.py), [operations](../scripts/check_operations.py), [transport](../scripts/check_transport_probe.py) and [T1](../scripts/check_t1.py). Inspect current arguments and select cases from the change; do not copy a stale command or infer a live pass from unit tests.

| Claim | Minimum evidence | Insufficient substitute |
|---|---|---|
| Safe read/acknowledgment | API, persistence, concurrency and restart tests | Bridge memory or a displayed unread count |
| Installed client works | Exact native installed artifact and real client profile | Source build plus harness-generated prefix |
| Agents use findings | Unscripted real-model task and observable artifact consequence | Prescribed calls or note counts |
| Change delivered | Reviewed exact patch, accepted event, actual apply and independent checkout check | Accepted event or worker summary |
| Two-person collaboration | Distinct people/principals and two physical machines | Multiple processes or same-principal sessions |
| Public installation works | Independent fetch/verify/install from live origin | Local signing or successful upload |
| Public snapshot is appropriately scoped | Typed allowlist, consent review, synthetic leak tests and rendered inspection | Hash match or “noindex” |

Retain compact, redacted evidence and important failures in `research/`, update its index, and link each completed package to evidence in the release ledger. Record exact skipped/unavailable checks. No benchmark win, security certification, supported platform or client capability is claimed beyond the exercised boundary.

## 6. Research recommendation coverage

Every recommendation in the source has a destination or explicit deferral:

| Research IDs | Destination |
|---|---|
| K1, K2 | W5 guidance/hints; W3 installs only supported wording |
| K3, K4, K5, K6 | W2 retrieval/acknowledgment foundation, W5 projections |
| K7 | W7 coordinator pending; initial authorization inbox in W4 |
| K8 | W5 report and labels; refusal policy deferred to W9 |
| K9 | W7 local observations; holder-status design W9 |
| K10 | W2/W5 reader protections before routine peer context |
| K11, K12 | W9 explicit per-client/correction decisions |
| K13 | W0 fixture and W6 real-model qualification, repeated in W7 |
| K14 | W7 only needed identity/ticket semantics; other designs W9 |
| N1, N2 | W3 local names and race-safe client metadata |
| N3, N4, N5 | W4 human display/resolution; W7 remote labels and typed MCP resolution |
| N6, N7 | W7 invitation lifecycle and authority, W3/W5 client surface |
| V1, V2 | W4 views/principal viewer; W7 scoped viewers |
| V3 | W2 typed reads, W5 projections; extraction when W8/Polaris needs it |
| V4, V5, V8 | W8 consented export and actual recorded example |
| V6, V7 | W9 hosted/local graphical extensions |
| H1, H2, H10 | W1 distribution/site/docs; command docs stay synchronized through W5/W7 |
| H3, H6 | W3 recoverable setup/readiness |
| H4, H5, H8 | W4 person controls/permission/recovery; W7 refinements |
| H7 | W7 invitee path |
| H9 | W4/W6 review and apply; W7 full dry-run UX |
| H11, H12 | W7/W9 usage; W1/W6/W7 explicit platform/client qualification |
| X1, X2, X4, X5 | W5 schema/errors/compatibility; W2 revision/acknowledgment semantics |
| X3 | W3 bound launcher |
| X6, X7 | W5 opt-in diagnostics when justified; W9 refinements/Merak request |

## 7. Commit and completion protocol

Implement and commit one verified logical change at a time: contract/fixtures; read and acknowledgment storage; setup recovery; bound launcher; human views and permissions; agent context/reporting; qualification; remote identity/join; scoped viewer; public export/rendering. Each remains a coherent change with its relevant tests and documentation; this list does not require enormous all-at-once commits.

Stage only owned files/hunks and inspect the staged diff. Use concise `feat:`, `fix:`, `docs:` or `chore:` messages without attribution trailers. Do not push, rewrite history, deploy, publish artifacts, buy services, run paid models or reset homes merely because this plan exists. Carry out already authorized implementation independently; resolve an outstanding decision only when its dependent work requires it.

At each package completion, record commit, checks, exact evidence boundary and remaining decisions. Before declaring the first release complete, W1 through W6 must meet their actual acceptance criteria and the external first-user attempt must be recorded. W7 and W8 have separate completion claims; W9 remains deferred until selected.
