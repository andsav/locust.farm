# Independent review of the implementation plan, with client probes and prior art

Date: 2026-10-03. **Status: review findings and recommendations; nothing here is an accepted decision or implemented behavior.** Reviewer: Claude (agent), at the request of the repository owner. It concerns the [implementation plan](../docs/implementation-plan.md) as of commit `c8103b0` (the October 4 revision) and complements the earlier [adversarial review](../docs/implementation-plan-review.md). Product naming and marketing copy are out of scope at the owner's request.

## Method and evidence boundary

Two rounds. Ten independent reviewers read the plan and research through separate lenses (protocol, security, scope, first-contact comprehension, user journey, research audit, client integration, substrate, and two prior-art sweeps). Nine second-round reviewers then tried to refute the main objections, checked cited facts against primary sources, and opened each prior-art candidate's repository.

The first round read the plan at `609edd2`. The plan was revised at 14:20 (`07510fe`) while that round ran; the second round read the revision. Findings below are re-baselined against the current text, and first-round claims the revision already answers are listed under "Corrections to the first round" at the end rather than repeated.

| Evidence level | What it covers |
|---|---|
| Run today on this machine | Codex sandbox probes (`codex sandbox -P :workspace`, codex-cli 0.153.4); a stdio MCP child under Codex reaching a Unix socket; `claude auto-mode defaults` rule dump (Claude Code 2.1.280); Git probes for symlink checkout and filter execution; GitHub metadata for every project named |
| Read today from primary sources | Claude Code and Codex documentation and Codex source; Iroh, iroh-blobs, p2panda and Radicle releases, READMEs and source; candidate repositories' docs and source |
| Reasoning from the plan text | Protocol, scope and security objections, each then attacked by a second reviewer |
| Not done | Nothing was built. No candidate tool was installed or run. No wake mechanism was exercised end to end. Whether Claude Code's classifier blocks an agent-run export was not observed (the standalone CLI here is logged out) |

This machine is not a default-configured test bed: `~/.claude/settings.json` sets `bypassPermissions`, and `~/.codex/config.toml` sets `danger-full-access` with approvals `never`. None of the client failures below reproduce in the owner's own sessions.

## Verdict

Nothing found has to be settled before any code is written. The second round could not sustain any first-round blocker at that level. The objections that survive are of three kinds:

1. **The date and the full scope are both fixed, and nothing says what gives.** A verified list of machinery the two-machine release cannot exercise exists (section 2); the plan currently keeps all of it.
2. **Three agent-facing defaults fail on default-configured clients** in ways the owner's machine hides (section 3).
3. **A handful of cheap contract decisions are unmade** and shape the first schema, envelope and CLI (sections 4 and 5).

The problem is not immediately understandable from the plan, and the plan cannot yet say why two collaborators would choose this over tools they already have (sections 6 and 7).

## 1. Date and scope are both fixed

The plan sets the release for the night of October 4, keeps "the core daemon, P2P board and context, workspace contributions, CLI and one-prompt skill installation in scope", and forbids silently dropping a requirement. The repository is one binary that prints `locust`. A second-round count found about 58 exit-evidence clauses and 15 conformance rows.

The date is the owner's constraint and is not argued here. What is missing is the rule for the evening of October 4, when some gates are red:

- **A short release-blocking list.** Suggested: the six-step flow of plan section 1 on two real machines; a non-member is refused events and blobs; materialization cannot escape its root; the installer does only what the prompt says; the packaged binary writes, reads and restarts on a fresh database.
- **An ordered drop list**, applied top-down when a checkpoint is missed. Section 2 supplies the candidates.
- **Who decides.** Otherwise the decision is made late, under pressure, possibly by an agent.

## 2. What the two-machine release cannot exercise

The release evidence path uses exactly two daemons. Each item below was defended by a second reviewer told to find the guarantee it protects in plan section 1 or M6; none was found. Each cut needs a recorded scope change and a reworded gate, as the plan requires.

| In scope today | Why it protects nothing in this release | What must survive the cut |
|---|---|---|
| Transactional outbox (M1) | Under the plan's own reconciliation model the signed event is the outbound intent; a peer that lacks it pulls it | Event append and projection update in one SQLite transaction; one durable pending row for invitation redemption, which happens before the joiner may reconcile |
| Content encryption, key distribution and rotation (M2) | Every holder of goal data is a member, transport is authenticated and encrypted, and retention peers are deferred. Rotation adds nothing: a peer that has not learned of a revocation has not learned the new key either | Sessions admitted by member endpoint key; "a non-member or revoked endpoint asking for an event range or a blob by hash is refused" becomes the load-bearing M2 gate; reserve envelope fields |
| TTL leases with renewal, three clocks, enforced retry budgets, separate assignment, attempt and claim-request IDs (M3) | Nothing heartbeats in an interactive model session. Read literally, a daemon restart or one long tool call makes finished work unsubmittable, which collides with M6's restart recovery | An instance handle plus a generation, with explicit takeover. This still answers the MoltMesh same-identity finding |
| Client-held delivery cursor for work discovery | A model session has no durable place for it. "Assignments for me that are not terminal" cannot skip work | A daemon-held cursor for the informational feed only |
| Canonical-encoding selection (M0) | Needed only if a verifier re-serializes | One encoding, strict decoding, sign and store the exact bytes, ID is their hash, test vectors |
| Revision bases and conflict inspection for every scratchpad document | Notes and findings are append-only; only plan and summary have a contested head | Accepted heads for plan and summary |
| `iroh-blobs` as the blob layer | Version 0.103.0 still calls itself not production quality and ignores per-request permission masks, so any peer that can connect can push blobs; the fix merged on October 2 and is unreleased | Depend on `iroh` 1.x alone; a small file store (temp file, fsync, rename, then the SQLite reference) with a hard size cap, or Git objects |

Three cautions from the second round:

- **The cuts are not all compatible.** "The log is the outbox" assumes two histories. A single coordinator-sequenced log is a larger simplification that the two-daemon path cannot distinguish from two histories, but it needs a worker submit queue, lets the coordinator omit contributions, and with three or more peers and a sleeping coordinator laptop it stops worker-to-worker exchange. That is an owner decision for plan section 1.
- **Dropping encryption and adding a store-and-forward mailbox are mutually exclusive.** A mailbox creates the non-member holder that encryption exists for.
- **Workspace transport must be chosen once.** The first round proposed four incompatible forms (bundles over one stream, Git fetch into a daemon-owned repository, namespaced refs in the participant's repository, a filtered manifest). If Git objects are used: fetch does not resume; LFS and submodule content does not travel; and Git does not enforce path containment. In a probe, `fsck --strict` and `worktree add` accepted and checked out symlinks pointing outside the tree, and checkout ran a filter program selected by the tree's own `.gitattributes`. A tree validator and isolated Git configuration are still required.

Two first-release gates cannot be exercised by two daemons and need a third identity: "a disconnected coordinator permits contribution exchange", and revocation where an honest member cites a revoked member's later event.

## 3. Client integration: verified facts

| Plan default | What was found | Level |
|---|---|---|
| CLI reaches the daemon over a Unix socket "through the shell capabilities coding agents already have" | In Codex's default workspace sandbox a process gets `EPERM` connecting to a Unix socket, even one inside its working directory; DNS does not resolve; writes to the home directory are denied. The default applies after the normal trust prompt | Run |
| | The small CLI-only fix is one Codex rule, `prefix_rule(pattern = ["locust"], decision = "allow")`. It removes the sandbox for `locust` entirely and does not match invocations with pipes, redirection, variable prefixes or an absolute path | Read; rule check run |
| | A stdio MCP server configured in Codex runs outside the command sandbox and reached the socket. Its tool calls prompt unless annotated read-only or non-destructive; its environment is filtered (no `XDG_RUNTIME_DIR`); tool timeout is 60 s in the docs and 300 s in source | Run |
| | Claude Code's Bash sandbox is off by default. When on, Unix sockets and UDP are blocked; local MCP servers, hooks and plugin monitors run with full access | Read |
| Active sessions "wait for work" | Claude Code: a foreground command is cut at 2 minutes by default (10 maximum) and moved to the background; a background command re-invokes the session when it exits. Background tasks are lost on resume and can be reaped in an idle session, so the cursor must live in the daemon | Read; wording from the installed tool definition, not exercised |
| | Claude Code channels cannot be a default: research preview, launch flag with a warning dialog, organization policy, silently dropped when off | Read |
| | Codex: asynchronous hooks, `notify` and background terminals cannot wake an idle session. Two immature paths exist: a synchronous Stop hook that blocks and continues the turn, and an experimental `codex queue` | Read; neither exercised |
| | In every comparable tool, "CLI plus skill" is the degraded tier. The only project with continuous-integration evidence of delivery into both clients ([hcom](https://github.com/aannoo/hcom)) uses per-launch hooks and a launch wrapper, and wakes idle agents by typing into a terminal it owns | Read |
| Install by pasted prompt | Claude Code starts in auto mode from v2.1.283. Its default rules gate running code from an external source and adding persistence unless the user's own message names the source and the mechanism. A pasted prompt is the user's message, so the prompt must literally name the URL, the install location and the service mechanism. In Codex each step is an approval | Read; prompt not tried |
| Agent-run export and result submission | Claude Code's one hard-deny rule concerns sending private-repository content to a destination not listed as trusted. Whether an opaque `locust export` trips it is unknown. An allow rule for `locust` resolves before the classifier | Read; untested |
| "Portable skill" | Same format, different locations (`~/.claude/skills`, `~/.agents/skills`). Both clients have plugins that bundle a skill, an MCP server and hooks; Codex's loader also accepts `.claude-plugin` manifests | Read |

Consequences for the plan:

- **Move a stdio MCP front-end into the release scope**, at least for Codex. The plan still lists MCP as deferred in section 1, section 6 and the later milestones; [agent-agnostic integration](agent-agnostic-integration.md) recommends the bridge without moving it. MCP gives the agent Locust's operations. It does not start a turn in an idle session and does not confine the agent's other tools.
- **Specify `wait` per client** rather than as one blocking call: a bounded mode with a timeout below the smallest client limit and a distinct "nothing yet" exit code, an exit-on-first-event mode, and a hook mode. Wake output should carry daemon-authored identifiers only, never peer-written text. Treat "needs an explicit resume" as the normal case.
- **Run the first real-agent slice on default configurations.** On this machine it will pass where a new user's will not.
- **Add `locust hook <event>`.** A hook subcommand of about 190 lines is how [AWP](https://github.com/agentwireprotocol/awp) surfaces inbound work between tool calls; it is also the known way to show a busy worker a durable cancellation request.
- **Merak.** A native adapter owns tool dispatch and subprocess launch, so it is the one integration where a worker policy can be enforced rather than described; see [local sandbox integration](merak-native-local-sandbox.md). `merak --prompt` defaults to permission mode `sudo`, which a worker lane must not inherit.

## 4. Decisions for the opening-pass contract

Each is wording plus a default, not a subsystem. The second round judged these sufficient for an invitation-only release among people who chose to work together.

1. **Who decides.** Add a person, agent or daemon column to the M0 permission matrix. Suggested: the person decides share scope, invitation, join, and accept-and-advance-head; at join the person gives the worker a standing instruction, "ask me before each task" by default.
2. **What joining authorizes.** A coordinator-signed assignment is a request for work inside the goal's directory. It never counts as the local person's consent and never widens the session's permissions. Member-authored text is shown as quoted data with its author key.
3. **Export.** A snapshot is the tree of one named commit with no history. Restore the proposal's default-deny line, which the plan dropped. The goal records one export root; export or artifact upload outside it is refused. The same applies to result patches.
4. **File access.** The CLI reads and writes workspace files. The daemon touches only its own state directory and never opens a path supplied by a client.
5. **Principals.** Neither client's default sandbox protects same-user credential files (`~/.ssh` was listable inside Codex's default sandbox). Either state one principal per daemon and drop enrollment from M1, or keep per-client credentials for attribution only and say so.
6. **Materialized trees.** A materialized tree is as trusted as a repository cloned from its author; instruction and configuration files in it will affect the session that works there. Limit v1 manifests to regular files, directories and the executable bit.
7. **Invitations.** Single use, short expiry, bound to the joiner's key at redemption; show the inviter the joiner's key fingerprint.
8. **Goal selection.** Key every table by goal ID and require every call to name its goal, or declare one goal per daemon. The plan forbids an implicit administrator but not an implicit current goal.

First-round fixes judged over-engineering for this release: a sandboxed `locust work` launcher, a separate owner socket with an approval queue, session-start hooks, OS notifications and peer presence.

## 5. Protocol gaps the plan does not answer

- **Restoring or copying state forks an honest signer.** A backup restore, a migration rollback after the new binary has signed, or a state directory copied to a second machine makes a daemon sign a second successor. The plan detects conflicting coordinator successors and halts, with no stated scope and no exit short of a new goal. [Iroh Rooms](https://github.com/kortiene/iroh-room) shipped an unscoped fail-closed rule, found it permanently disabled rooms after ordinary concurrency, and reversed it on its main branch (ADR-0005; the reversal is in no release). Add a signing high-water mark kept outside the main database, or adopt own events from peers before the first signature after start; an exclusive lock on the coordinator store; the scope of a halt and the decision that ends it; and one restore scenario in the conformance matrix. The earlier review's request for coordinator key backup raises this hazard and should be paired with it.
- **Events that depend on excluded events.** After a revocation frontier, an honest member's event may cite a revoked member's later event. State that dependencies require presence and structural validity only; excluded events are stored and relayed but not projected.
- **A follower that cannot apply a signed decision** must enter the same visible halt as a fork and never drop the entry.
- **Task lifecycle.** Nothing says who signs `offered`. Smallest answer for the release: a named assignee, creation and assignment in one decision, and one acceptance decision that may carry the new head.
- **Removal of content.** The likeliest incident is a secret in a note, snapshot or patch. If signed bytes contain the payload, removing it breaks every successor. Sign the payload hash, never the payload; add a redaction decision that cooperating peers honour; define `leave`.
- **Default relay.** With Iroh a relay is the normal rendezvous for two peers behind NAT, not a fallback. The default preset uses relays run by Iroh's vendor and publishes each endpoint's ID and home relay there; the vendor describes its public relays as suitable for development and testing. Name the default operator, carry the relay URL in the invitation, and say what third parties learn.
- **Two laptops and no store-and-forward.** State crosses only while both daemons are online. The release scenario passes while ordinary asynchronous use fails. Either show it ("last synchronized with Bob 9 h ago; later changes unknown") or accept a mailbox and keep encryption.
- **Sleep and wake.** The monotonic clock stops during macOS sleep. Persist deadlines as wall-clock time and force reconnection on wake.

## 6. Understandability and user experience

**The category is understandable; the problem is not.** After the README and plan section 1, a first-time reader could say what Locust is and could not say whose problem it solves, what they do today, or why they would switch. The words "problem", "why" and "today" do not occur in the plan's 5,300 words. The clearest statement of the product is the M6 exit evidence. The earlier review contains a problem statement; the plan does not.

A proposed opening:

> Developers who each run their own coding agent on their own machine cannot hand work between those agents: the humans relay tasks, notes and patches by hand through chat, issues and pull requests, or everyone moves into one vendor's hosted workspace. Locust is a small local daemon plus an agent integration that gives those agents one shared task board, shared notes and patch exchange for a goal, synchronized directly between the participants' machines. Nobody else hosts the board, and each person's agent runs only on their own machine, under their own sandbox, credentials and model subscription.

**The experience between "installed" and "first task accepted" is not designed.** Walking plan section 1 with four actors stalls at six points: how the invitation travels and what the invitee consents to; where the invitee's files land relative to their agent session (the install session is not the work session); who approves share scope, taking a task and accepting a result; how peer-written task text becomes something the worker acts on; how either agent waits; and how either person learns that something happened.

**Concept load.** Section 6 proposes about 38 operations; each agent needs about 15 before the first task is accepted. Comparable agent boards use four verbs (`ready`, `show`, `claim`, `close` in Beads). "Accept" names five different acts in the plan and "claim" three, and these become command and event names read by a model. Define ten or fewer everyday commands from a target transcript and keep the rest behind `--json`.

**Accepted head and Git.** Nothing answers "is the other person's change in my repository?". Suggested: a snapshot is a commit; a result is commits on that base, fetched into a namespaced ref; acceptance is the coordinator decision plus an optional merge, as one confirmed step.

A target first session to design backwards from:

```text
ALICE (in her repository)
alice> <pastes the install prompt>
agent> Locust installed and verified. Daemon running. Nothing is shared yet.
alice> Start a goal "add OAuth login" from this repo and invite Bob.
agent> I will share 212 tracked files at commit 4f2a9c1 (no untracked files). OK?
alice> yes
agent> Goal created; this laptop is its coordinator. Send Bob this one-use invitation: ...

BOB (any directory)
bob>   <pastes the install prompt, then the invitation>
agent> Invitation to "add OAuth login" from Alice (key 7QK4-91TF). Joining copies 212 files
       to ./add-oauth-login. You share nothing until you submit a result. Tasks run on this
       machine, on your model account. Join?
bob>   yes, and ask me before each task

ALICE
agent> Bob joined (key 9MX2-C0PD). Proposed: T1 token storage (me), T2 callback handler (Bob). Assign?
alice> yes

BOB
agent> Task T2 from Alice: "OAuth callback handler + tests" on 4f2a9c1. Take it?
bob>   yes
agent> T2 submitted: 3 files, 14/14 tests pass. Waiting for Alice's review (not accepted yet).

ALICE
agent> Bob submitted T2, on branch locust/T2-bob here. Accept and merge into main?
alice> accept
```

## 7. Why this instead of what exists

The plan's demonstration (assign, patch, review, accept between two collaborators) is the scenario existing tools already serve. Read today from each project's documentation and source; none was run.

| Project | Who hosts the shared state | Works when never online together | Reaches an agent that is not waiting | Authority over acceptance |
|---|---|---|---|---|
| Pull requests on a shared forge | The forge | Yes | Hosted agents, or a person | The maintainer's merge |
| [Beads](https://github.com/gastownhall/beads) | Nothing extra; the Git remote the collaborators share | Yes | No; a session-start hook primes each session | None; assignee is free text and anyone with push access can rewrite the board |
| [Multica](https://github.com/multica-ai/multica) | A central server, vendor-run or self-hosted | Yes; queued work waits for an offline machine | Yes; a daemon launches the agent, with permission prompts bypassed | None |
| [Buzz](https://github.com/block/buzz) | A relay one party hosts | Yes | Yes; a harness wakes the agent on mention | Signed issues and assignments; agents answer only their owner by default |
| Locust as planned | Nobody; a relay for rendezvous | No | No; the session must be in a wait loop | Signed coordinator decisions with explicit acceptance |

None of the others combines no host, authenticated authority with acceptance, and independent operators. That gap is real. The release as planned gives up the two things they already do. The properties that justify the protocol work are: the parties share no forge, account or storage credentials; content sits on no third-party host; one party holds signed authority over membership and acceptance; and clients from different vendors take part. The first demonstration should exercise one of these.

## 8. Projects worth a teardown

Nothing closer than MoltMesh exists on the full product shape. Closer references exist for individual layers, and many near-duplicates appeared in recent weeks (two within the last ten days), which suggests the problem is widely felt and the solution unsettled. Time-boxed reads, not five-document treatments:

| Order | Project | Read for | Effort | Caveats |
|---|---|---|---|---|
| 1 | [hcom](https://github.com/aannoo/hcom) (Rust) | The per-client hook table for Claude Code and Codex, the tested prompt wording for "end your turn to receive", acknowledging delivery only after the hook has flushed its output, the Codex rules file | 2.5 h | Idle wake is terminal keystroke injection with open bugs; its skill docs lag its source |
| 2 | [Iroh Rooms](https://github.com/kortiene/iroh-room) and [Jeliya](https://github.com/kortiene/jeliya) (Rust, Iroh 1.x) | Envelope and verification rules, key-bound invitations, the authority-fork reversal, the enforced cap of five members | 90 min minimum before the envelope is fixed | One person's agent-generated code (about 280,000 lines in eight weeks), dormant since August, nothing on tasks. Not a candidate to adopt or fork |
| 3 | [Beads](https://github.com/gastownhall/beads) (Go) | Per-replica lease rules, what a double claim does across replicas (a merge conflict that halts sync), integration through hooks and a short primer | 1.5 h reading, 1.5 h running | No identities, no acceptance step |
| 4 | [Multica](https://github.com/multica-ai/multica) (Go) | The product users will compare against: run versus issue states, the consent setting for who may put work on a machine, the cost of an unattended runner | 2 h | Source-available licence |
| 5 | [AWP](https://github.com/agentwireprotocol/awp) (Go) | The same daemon, CLI and skill shape; the hook subcommand; a wait contract with a distinct exit code | 1.5 h | Nine days old; verified by its author on Claude Code only |

Shorter reads: [Agent Relay](https://github.com/AgentWorkforce/relay) for its written delivery contract; Buzz for signed assignment without a coordinator chain and its agent etiquette prompt; [wattswarm](https://github.com/wattetheria/wattswarm) for a claim and lease event vocabulary (mostly AGPL: ideas only); [papo](https://github.com/Kelvin-Jesus/papo) for an `iroh-gossip` dialing pitfall. MCP Agent Mail's licence rider excludes model vendors and anyone acting for them, so a person should read it and copy nothing. Not worth the time: Wasteland (its public board is almost entirely back-filled from pull requests) and Radicle as a base (on September 23 it disclosed that node traffic in every released version is unencrypted and unauthenticated).

A template so teardowns are comparable: selection record and adoption facts; pinned commit; scope filter mapping the subject to plan sections 4 to 7; a clean-install transcript on two machines; how work reaches a session per client; one finding register with stable IDs, evidence level and plan anchor; what it gets right; measurements; not-verified list; a table from finding to plan change.

## 9. Research quality

- The MoltMesh teardown is accurate: eleven of eleven spot-checked claims hold at the pinned commit. As product research it is thin: the project has five stars and one contributor, so it yields a defect list and nothing about users or client behavior.
- Every MoltMesh defect became a first-release requirement without being ranked against a two-person release. Section 2 is that ranking.
- Four MoltMesh lessons have no gate in the plan: a reserved or plaintext event in an encrypted goal; size and count bounds on member input; a non-member fetching a blob by hash; a member who missed a key change. The third matters most if encryption is cut.
- The [landscape](landscape.md) is accurate where checked but omits what decides the stack: `iroh` is stable at 1.3.0 while `iroh-blobs`, `iroh-docs` and `iroh-gossip` are 0.x.

## 10. Process for the October 4 push

- **Parallel streams in one checkout.** Use a worktree and branch per stream, land the shared-types file first with one owner, merge through that owner after the workspace checks pass on the merged tree, and define "reviewed".
- **Gate evidence written by agents.** Keep one ledger: gate, evidence level, command, raw artifact, and who observed it. Define the evidence levels once.
- **Continuous integration** has one Ubuntu job. Add macOS (a release target), a release-build smoke test on a fresh database, and a two-peer test.
- **The day after release.** Define a log location, a redacted report bundle that two peers can join on event IDs, and a withdrawn or minimum version in the install manifest.
- **Licence.** Listed as a release prerequisite; no `LICENSE` file exists yet.
- **Client terms.** State that each participant authenticates their own client and that Locust never handles client credentials; record the authentication mode in the support matrix.

## Corrections to the first round

- About a dozen findings of the form "first exercised at M5 or M6" described the earlier plan. The revision moves real-agent runs to the first runnable slice, adds role playbooks, and lists licence and key custody as release prerequisites.
- "Codex has no inbound wake" was too strong; the supportable claim is that no tested, supported wake exists for an idle interactive session.
- "Agent-run sharing is classed as data exfiltration" is a hypothesis, not an observation.
- "Treating peer content as untrusted is incoherent for a task" was overstated: both client vendors take the position that a peer's request is never the user's consent. What is missing is one paragraph on what joining authorizes.
- The two lease sentences are ambiguous, not contradictory, and claim generations cannot be removed.
- The plan does not require coordinator acceptance for a scratchpad revision to exist or be read.
- The first round rated Wasteland highly and inferred that Buzz is building cross-operator replication; both were wrong on inspection.
- Recommendations sized in days or weeks, including a one-to-two-week estimate for log synchronization (the nearest comparable is about 1,640 lines), were written before the date was known.
