# Implementation plan review

Date: 2026-10-03. **Status: adversarial review of the proposed [implementation plan](implementation-plan.md); the plan itself is unchanged by this document.** No implementation exists yet, so every claim below concerns the proposal, not shipped behavior. Reviewer: Droid (agent), at the request of the repository owner.

## Verdict

The plan is unusually disciplined about status and evidence boundaries, and it contains no dishonest claims. The objections below concern sequencing, scope, and a few missing deliverables, not rigor. In order of severity: the product hypothesis is validated last, the first release is a multi-quarter program with no cut list, and the build-versus-adopt question is applied only to transport, not to the collaboration layer itself.

## Understanding of the project

**Goal.** A protocol plus a small Rust daemon that lets independently operated coding agents (for example Claude Code and Codex sessions) work on one shared goal together. Each participant runs a daemon that holds a signed, git-like record of the goal: tasks, offers, results, notes, artifacts. Daemons sync directly peer to peer, with replaceable relays where NAT demands them. A designated coordinator, one of the participants rather than a vendor, owns authoritative decisions: assignments, accepted results, membership, accepted heads. Execution, sandboxing, credentials, and what gets exported stay with each participant. Joining is by invitation. Onboarding is a prompt pasted into a coding agent plus an operating skill. It is explicitly not a model, sandbox, marketplace, or consensus system (`docs/implementation-plan.md` §1).

**Problem.** Multi-agent coding today works inside one vendor's cloud or one machine. Collaboration across people or organizations means adopting shared infrastructure you do not control (a hosted org, a cloud agent service, a message board), with no neutral coordination layer that preserves provenance, tolerates disconnection, and gives acceptance an explicit owner. MoltMesh is the evidence that the shape is wanted and that naive builds fail in specific, costly ways.

**Succinct explanation.** Locust lets two people's coding agents team up on one goal. A small local daemon keeps a signed record of the shared work and syncs it directly with the other participant's daemon, each agent runs in its own sandbox, and a participant-appointed coordinator accepts results. No vendor in the middle, no shared cloud account.

## Objections

1. **The riskiest assumption is tested last.** M0–M4 harden protocol and storage with deterministic workers; real coding agents and real people appear only in M5/M6. The primitives exist already (the [landscape survey](../research/landscape.md) shows that); the open questions are behavioral: do agents collaborate usefully in a wait/work loop, do humans want this, and does it beat a shared Git remote plus two agent sessions? *Change:* run a timeboxed product spike before M1. Two real agent sessions (one Claude Code, one Codex), one deliberately dumb shared board (a git-tracked JSON file with a polling CLI is enough), one real task end to end. That spike can kill or reshape the design; the daemon cannot.

2. **The first release is a multi-quarter distributed-systems program with no cut list.** M2, M3, and M4 each contain what other projects treat as a product: reconciliation, membership epochs and key distribution, leases and attempt generations and cancellation, workspace materialization. There are no estimates, no per-milestone timeboxes, and no explicit "not in v1" list beyond deferred scope. *Change:* write the cut list before M1. Candidates: one attempt generation; revocation means only "stop future assignments plus rotate future keys"; scratchpad documents are single-writer; no retention receipts; conflicting decision chains halt and export rather than auto-resolve. Record each cut as deliberate debt with a target milestone.

3. **Build-versus-adopt applies only to transport.** The plan forces an Iroh evaluation, but it presupposes a bespoke signed event log, coordinator hash chain, membership and encryption scheme, and reconciliation protocol. p2panda, iroh-docs, and Radicle already cover parts of that, and MoltMesh's failures came from bespoke protocol work done under time pressure. *Change:* M0 includes a timeboxed adopt-versus-build spike against this plan's own acceptance scenarios (offline reconcile, revocation, restart), with adoption as the default unless it fails for a stated reason.

4. **The wait-loop dependency needs day-one measurement.** Skills cannot interrupt an ongoing tool invocation, clients differ on reload and push behavior, and sessions have turn, context, and timeout limits. If a Claude Code or Codex session cannot realistically stay parked on `locust wait` and take work when it arrives, the active-worker model collapses and the M5 caveat becomes the product. *Change:* measure both claimed clients in the product spike, then design notification semantics around the measurement. Treat "needs an explicit resume" as the default mode, not an edge case.

5. **Coordinator model: two gaps.** Section 4 of the plan is honest about the availability trade-off, but (a) "explicit resolution" of conflicting decision chains is unspecified: define the operator procedure, out-of-band head comparison, resolution decision, or fork declared as a new goal. (b) Coordinator key backup and export must be an M2/M3 exit criterion. Losing one laptop currently means losing the goal, which undercuts the resilience story the product is selling. Also watch the wording: "no mandatory central service" must not read as "always available"; the coordinator's uptime is the goal's authority uptime.

6. **No threat model deliverable.** There are scattered mitigations, but the system's core act is delivering other people's model-generated content into your agent's context. *Change:* add an artifact before the wire format freezes. Adversary list (malicious peer, malicious coordinator, stolen member key, same-OS-user local process, already conceded as partial), abuse cases (prompt injection through tasks, notes, and artifacts; event and artifact flooding; invite spam; result poisoning), controls (quotas, caps, labeling all peer text as untrusted in CLI and skill, quarantine of unreviewed artifacts), and tests mapped into the section 9 matrix.

7. **Add deterministic simulation to the verification strategy.** Crash injection at durability boundaries is planned and good, but partition, reordering, and clock-skew behavior is only implied by "disconnect/reconnect" tests. *Change:* build an in-process multi-daemon harness with virtual time and network (turmoil or a small custom equivalent) so the section 9 conformance matrix is CI-executable. Keep two-real-networks and real-agent runs as manual release gates with retained evidence, but do not leave reconciliation logic tested only by those.

8. **Milestones are bundles, not slices.** M0 bundles a spec, fixtures, a transport experiment, seven binding decisions, measurements, and version selection. M5 bundles the installer, service adapters, the skill, migration, uninstall, reload and wait-loop tests. *Change:* split each into individually reviewable commits and artifacts, explicitly mark throwaway spikes versus kept slices, and require a runnable local demo at each milestone exit. Move the normative content into a versioned protocol spec at M0; the 40 KB plan should shrink to a tracker so it does not rot.

9. **Process basics are missing.** The repository has no LICENSE file despite "open source" in the [README](../README.md) and on the site; no one owns release signing key custody and rotation, even though the one-prompt install trusts that root; there is no hosting or CI budget for relays and the platform matrix; there are no go/no-go criteria between milestones; and there is no maintainer-loss story, which is notable for a tool whose pitch is the absence of central authorities.

10. **Day-one UX and role playbooks are absent.** The plan specifies a protocol but not the opinionated workflow: what the coordinator agent should do (decompose the goal, write task specs with review criteria) and what the worker agent should do (claim, execute, submit evidence). *Change:* prototype default role prompts and goal/task templates in the product spike and ship them in the skill. Without them, beta evidence will be weak.

11. **Public site drift.** The landing page [`sites/locust.farm/src/routes/+page.svelte`](../sites/locust.farm/src/routes/+page.svelte) advertises `locust swarm --goal "…"`, a command that does not exist, under a "Distributed Agent Swarm" framing the plan does not use. *Change:* gate that copy before M5/M6 and reconcile the naming with the invitation/goal/coordinator model.

## Strengths of the plan

- Honest status labeling and evidence boundaries throughout; proposals, research findings, and verified results are kept distinct.
- Deterministic worker before model agents, so failures can be isolated from model behavior.
- A single coordinator instead of premature consensus or timeout-based failover; offline contributions stay exchangeable while authoritative changes pause.
- "A remote request is an offer, not a command": peer content never authorizes execution.
- Crash-boundary tests, idempotency scoping, transactional outbox, and the section 9 failure matrix.
- Refusing size and availability slogans before measurement, and full research traceability for adopted ideas.

## Gates to attach before M1 starts

1. Product spike done, or explicitly skipped with a written rationale.
2. Cut list, timeboxes, and go/no-go criteria written into the plan.
3. Adopt-versus-build decided for the collaboration layer with evidence.
4. LICENSE, release key custody, and relay/CI budget decided.
5. Wait-loop behavior measured on both claimed clients.

If any objection here is accepted, record the resulting change in the [implementation plan](implementation-plan.md) and link back to this review; if rejected, note the rationale there so the decision survives.
