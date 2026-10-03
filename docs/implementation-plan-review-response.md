# Response to the second independent review

Date: 2026-10-03. **Status: rationale for amendments to the proposed [implementation plan](implementation-plan.md), not implemented behavior or release qualification.** Reviewed the [independent review](../research/implementation-plan-independent-review.md) against the plan and current primary documentation. The October 4 target and core product scope remain. No code, client configuration or sandbox was changed.

## Assessment

The review is strongest where it exposes an unstated contract: default client IPC, worker lifetime, restore behavior, consent, export and release evidence. Its two-person demonstration is too narrow a basis for declaring protocol protections unnecessary. Adopt the useful simplifications by preserving their invariants; do not infer that unexercised behavior has no value.

The review reports local client/Git probes, but their raw commands/results are not retained in a corresponding tracked evidence appendix. Treat them as reviewer-reported observations until reproduced in the release ledger. Current primary documentation supports the stdio bridge direction. This response did not rerun those probes or verify the review's entire competitor survey.

## Incorporated into the plan

| Review finding | Plan amendment |
|---|---|
| Default shell access does not establish daemon connectivity | Include a thin stdio MCP bridge in the first slice/release; qualify default client profiles and preserve the owner's configuration. CLI remains available where permitted. |
| Wait/wake behavior differs by client | Make manual resume the baseline; caller-selected waits, explicit no-event/disconnected results and durable daemon-held delivery state. Hooks and automatic wake are separately qualified. |
| Who consents and what joining means are unclear | Add participant/agent/coordinator/daemon responsibility, explicit goal IDs, invitation redemption rules, destination/export scope and standing execution grants. Existing authorization persists. |
| Interactive leases have no heartbeat owner | Use durable session-bound claims and explicit authenticated generation takeover; retain stale fencing, deadlines/retry policy and coordinator supersession. Expiring leases need a resident renewing adapter. |
| Work discovery must survive a vanished session | Reconstruct work/cancellation from durable task state, separately from notification cursors. Do not rely on model memory for correctness. |
| Restore can fork an honest signer | Supported restore/rollback enters read-only recovery; define per-goal conflict halt, operator retirement of the old writer and explicit new-goal fallback. Test copied-state equivocation without claiming all raw copies can be detected in advance. |
| Excluded ancestry and valid authorization are different | A valid excluded event may satisfy a causal reference, but cannot authorize membership, assignment or an accepted base. Missing, unsupported, conflicting and I/O-failed states differ. |
| Workspace representation and export need precision | Choose manifest/content snapshots and base-bound patches, default commit selection without history, explicit uncommitted inclusion, reviewed export scope and a restricted file-type set. Daemon storage never opens arbitrary client host paths. |
| Git integrity does not establish safe materialization | Validate trees/paths, reject unsupported links/types and avoid automatic Git filters/hooks. Keep submitted, accepted and locally integrated states separate and recoverable. |
| A two-machine happy path misses three-party faults | Add three local daemon instances for coordinator outage and revoked-author ancestry; retain the physical two-machine/client test. |
| Release authority and evidence are too implicit | Name the repository owner as go/no-go authority; add the [single evidence ledger](release-evidence.md), default-profile qualification, macOS CI requirements and operational diagnostics. |
| Product benefit is not exercised | Use mixed clients and independent accounts without a shared forge dependency. Show overlapping-daemon availability and synchronization age honestly. |

## Simplify implementation without deleting guarantees

- **Event log as outbound intent:** allow it when reconciliation can rediscover every committed record. Keep atomic event/projection/idempotency updates and durable pending records for obligations outside that log, including pre-membership redemption. This does not collapse contributions into a coordinator-only history.
- **Exact-byte signatures:** retain and verify the signed bytes rather than reserializing them. Still specify one versioned encoding, unambiguous decoding, signature domain and byte/hash/signature fixtures.
- **Append-only notes:** use correction/supersession references for notes/findings; preserve revisions and expected-base checks for replaceable plan/summary documents.
- **Content removal:** keep detachable user payloads behind hashes and define local withdrawal/leave now. A coordinated redaction protocol is deferred; it must preserve replay-critical metadata and cannot guarantee erasure from peers/backups.

These are proposed implementation defaults. They reduce duplicated machinery without weakening durable history, authorization or local ownership.

## Claims qualified or not adopted

**No automatic security/scope cuts based on the demo size.** Keep private content encryption and membership/key-change tests. Transport-only confidentiality could be a deliberate narrower threat model, but “encryption protects nothing” does not establish that choice. A future non-member mailbox would further change the requirements. The short release checklist does not waive detailed gates; the owner decides any scope revision.

**No broad unsandboxed Locust prefix rule.** The Codex [rules documentation](https://learn.chatgpt.com/docs/agent-configuration/rules) says allow rules authorize matching execution outside the sandbox. Use the narrow typed bridge and test its own authority. The review's universal claim about pipelines not matching rules is also too broad: current documentation describes splitting some simple shell chains.

**No universal wake claim or fixed shortest-client timeout.** Codex [hook documentation](https://learn.chatgpt.com/docs/hooks) distinguishes async completion from starting a turn. Claude's [hooks reference](https://code.claude.com/docs/en/hooks#command-hook-fields) now documents `asyncRewake`; this is another candidate, not tested Locust readiness. Publish behavior for exact supported versions/configurations rather than freezing reported timeout numbers into the protocol.

**Merak is not the only enforceable integration.** Pi extensions plus an OS boundary or a supervisor around an external client can satisfy a local policy too. Claude's [sandbox documentation](https://code.claude.com/docs/en/sandboxing#what-runs-outside-the-sandbox) explicitly distinguishes its Bash-only sandbox from enclosing the whole process. The [agent-agnostic design](../research/agent-agnostic-integration.md) remains the integration contract; whole-worker sandbox qualification is separate from MCP connectivity.

**Scoped credentials are more than attribution, without being same-user isolation.** They still constrain ordinary API use and accidental/confused-deputy access. Keep them and state the credential-theft boundary. Similarly, a daemon receiving bytes cannot prove those bytes came from a permitted export root; that needs a trusted file adapter or sandbox.

**A separate high-water file is not complete rollback protection.** It can be copied/restored too. Reconciliation cannot prove no unseen successor exists when every witness is unavailable. Managed recovery and explicit limitations are required; do not promise transparent signer continuity.

**Do not replace a blob stack on an unverified version assertion.** The review's specific `iroh-blobs` vulnerability/fix timeline was not independently established here. M0 now checks pinned transport and blob components separately, including direct fetch/push authorization, retention and crash behavior. A temp-file/rename store alone does not implement authorized resumable network transfer. Iroh's [1.3.0 documentation](https://docs.rs/iroh/1.3.0/iroh/) does confirm the relay can be involved in ordinary connection establishment, which is reflected in the plan.

**No arbitrary command-count cap or mandatory teardown schedule.** Design a short happy path while keeping the complete typed API. `--json` changes representation, not operation availability. Prior art such as hcom/AWP can answer a concrete adapter question during implementation; another broad survey is not a prerequisite to write code.

The existing plan already required early real-agent runs, a license, signing custody, packaged fresh-database checks and honest manual-resume reporting. These were assigned clearer evidence/ownership rather than rediscovered as missing requirements. Licensing remains the repository owner's choice.

## Ready to start

Proceed with the shared API/event fixtures, daemon persistence and stdio bridge in parallel with installer/playbook work. Resolve pinned encoding/crypto/transport choices in the opening implementation pass and record them; do not wait for every optional adapter or prior-art investigation. The first useful evidence is a real default-profile client operation and recoverable task flow. Documentation checks validate these amendments, not any runtime guarantee.
