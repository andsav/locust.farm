export const meta = {
  name: 'backup-host-design-round',
  description: 'Write one design for the first version of replacing a host (one optional backup host), attack it for safety and for fit with the code, revise it, and attack the revision',
  phases: [
    { title: 'Design', detail: 'one design as implementation phases', model: 'opus' },
    { title: 'Attack', detail: 'safety adversary; code and product fit', model: 'opus' },
    { title: 'Revise', detail: 'the designer answers every break', model: 'opus' },
    { title: 'Verify', detail: 'the adversary attacks the revision', model: 'opus' },
  ],
}

const REPO = '.'
const S = 'scratch'

const COMMON = `Repository: ${REPO} (Rust workspace "Locust"). Read-only: do not edit, create or delete anything in the repository, do not run cargo, npm or any build, do not commit. Other sessions are editing this checkout.

Read first:
- ${S}/owner-decisions.md: what the owner decided. These win over everything. (A scratch file; never cite its path.) In particular: design for a host that disappears, is asleep, or is restored from an old copy, not a hostile host; hostile members are the host's to remove; agreement among several people only when a host is replaced; first version is ONE backup host named in advance who can take over alone; naming one is optional and nothing asks for it; if the host removes the backup and the backup takes over without having heard, the host's removal wins and the takeover is void when the removal surfaces; a host may name as backup someone who came through a public door, by one explicit command; no migration (this piece takes its own protocol number and ends goals made before it); ergonomics with no friction come first.
- ${REPO}/docs/master-plan.md: the decisions in one place and the build order this piece follows.
- ${REPO}/research/replacing-a-host-2026-10-05.md, all of it: the groundwork, four designs that reviewers broke, what survived (section 6), the open problems with the fixes proposed (section 7), the smallest sound first version (section 8), what to model (section 9).
- ${REPO}/research/host-key-failure-characterization-2026-10-05.md and ${REPO}/research/goal-lifecycle-characterization-2026-10-05.md: measured behaviour. Among it: a member who built on a dropped record cannot use that key in the goal again; a survivor that holds a removal but not its new content key cannot write text; a removed computer is refused on every dial and never told.
- ${S}/safety.json (scratch; never cite its path): three plan pieces this design builds on, and the seam report between them, whose fixes apply. KEY (phase K1): every goal has a governance key of its own, kept on the host's computer; the first record names the governance key and the host's agent; the governance key signs governance and the host's daemon's own steps; the host's agent is an ordinary member that cannot be removed. GUARD (G1, G2): a daemon knows what it signed and, started from an older copy, signs nothing in the affected goals until it has caught up; the person's override is one command. END (E1 to E3): the host ends a goal with one record. Each piece names one place where replacing a host plugs in; read "plug_for_host_replacement" in each and the seam report's last entry.
- ${REPO}/docs/roles-and-permissions-plan.md: the form your phases take, the person's command grammar, which commands ask first, the one-member rule, the first-files rule, and Phases 8 and 9 (the host's daemon records the plan's text and file changes).
- ${REPO}/research/joinable-farms-rewrite-contract-review-2026-10-05.md, finding 12: what public goals need a takeover to settle.
Then read the code your design changes: crates/locust-core/src/goal/chain.rs, history.rs, fold.rs, workspace.rs, mod.rs, screen.rs; crates/locust-core/src/node/peers.rs, replica.rs, requests/goals.rs, requests/invitations.rs; crates/locust-core/src/sync/responder.rs, driver.rs; crates/locust-proto/src/event.rs, invite.rs, sync.rs; research/tla.

House rules: plain words and short sentences; define a term once and keep it; nothing every daemon must agree on is decided by clock, timeout, arrival order or lowest hash; everything shared is a pure function of the set of signed events held; local state lives only on the person's daemon; the person's commands follow the roles plan's grammar; ask as little of the person as is sound; greenfield, no compatibility layers. Say what you read in the code and what you inferred. Do not hand-wave a hard case: give the exact sequence of events and what every daemon computes.`

const PLAN_SCHEMA = {
  type: 'object',
  properties: {
    summary: { type: 'string' },
    user_explanation: { type: 'string', description: 'What a person needs to know, at most six sentences, in words that predict what happens when a host was only asleep' },
    choices: { type: 'array', items: { type: 'object', properties: { problem: { type: 'string' }, chosen: { type: 'string' }, cost: { type: 'string' }, rejected: { type: 'string' } }, required: ['problem', 'chosen', 'cost', 'rejected'] }, description: 'One entry per open problem of the research note: the fix chosen, what it costs, and the alternatives rejected with reasons' },
    phases_markdown: { type: 'string', description: 'Implementation phases in the roles plan\'s form, numbered B1, B2…, starting at heading level 3' },
    scenarios: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, sequence: { type: 'string' }, outcome: { type: 'string' } }, required: ['id', 'sequence', 'outcome'] } },
    safety: { type: 'string', description: 'What holds, as a property over deliveries; exactly when a takeover a daemon treated as in force can be undone' },
    liveness: { type: 'string', description: 'Who must be awake and what a person must do; when the goal is stuck and how it gets unstuck' },
    terminal_texts: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, title: { type: 'string' }, caption: { type: 'string' }, text: { type: 'string' } }, required: ['id', 'title', 'caption', 'text'] } },
    owns: { type: 'array', items: { type: 'string' } },
    changes_to_other_plans: { type: 'array', items: { type: 'object', properties: { plan: { type: 'string' }, statement: { type: 'string' }, becomes: { type: 'string' } }, required: ['plan', 'statement', 'becomes'] } },
    model_first: { type: 'string' },
    cleanup: { type: 'array', items: { type: 'string' } },
    verification: { type: 'array', items: { type: 'object', properties: { behavior: { type: 'string' }, check: { type: 'string' } }, required: ['behavior', 'check'] } },
    later: { type: 'array', items: { type: 'string' }, description: 'What is left for a later step (for example a threshold among several named people) and the slot it will use' },
    owner_questions: { type: 'array', items: { type: 'string' } },
    answered_breaks: { type: 'array', items: { type: 'string' }, description: 'In a revision: each break and what changed for it, or why it stands as a stated limit' },
    unsure: { type: 'array', items: { type: 'string' } },
  },
  required: ['summary', 'user_explanation', 'choices', 'phases_markdown', 'scenarios', 'safety', 'liveness', 'owns', 'changes_to_other_plans', 'model_first', 'verification', 'owner_questions'],
}

const CRITIQUE_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string' },
    breaks: { type: 'array', items: { type: 'object', properties: {
      claim: { type: 'string' }, steps: { type: 'string' }, severity: { type: 'string', enum: ['fatal', 'serious', 'minor'] }, fix: { type: 'string' },
    }, required: ['claim', 'steps', 'severity'] } },
    holds: { type: 'array', items: { type: 'string' } },
    unverified: { type: 'array', items: { type: 'string' } },
    recommend: { type: 'string' },
  },
  required: ['verdict', 'breaks', 'holds', 'recommend'],
}

const TASK = `Write ONE design for the first version of replacing a host: one optional backup host. It is the design the earlier round did not produce: that round compared four families, its reviewers agreed on a skeleton (a takeover is one record that names the exact last host record it builds on, signed by a fresh key; what the old key signed after that is set aside even if delivered later; work built on set-aside records is excluded without stranding its author), and they left competing fixes for the hardest problems. Choose one fix per problem, say what it costs and why the others lose, and write the result as implementation phases.

Settle every one of these with the exact rule and the exact sequence that shows it:
1. Naming, un-naming and changing the backup host, as records of the governance key. That naming is optional, that nothing asks for it, and what status says when none is named (a fact, never a prompt).
2. The planned handoff: a host who is present gives the seat to someone. Same record or another, and what the old host keeps.
3. The takeover record: what it names, which key signs it, how the new governance key is made (the research offers a fresh random key and a key derived so that a repeated takeover is byte-identical), and what the backup's daemon does before it signs (what it must hold, whom it must have heard from, how this meets GUARD).
4. Who may take over from which record. The owner decided that the host's removal or un-naming of the backup wins even if it surfaces after the takeover. Give the validity rule as a pure function of the held events, say exactly when a takeover that a daemon treated as in force is undone, and what then happens to everything the backup did as host (admissions, removals, rule changes, the plan's text, landed file changes) and to the members who built on it.
5. Two takeovers of one term, including one backup's computer restored from an old copy taking over twice. It must not freeze the goal for good.
6. What a takeover sets aside, and removals. A removal the backup never received is dropped by the takeover, which lets a removed member back in and reopens reading to it. Say what the host is told about which of their removals the backup has seen, and what the new host's daemon does first.
7. Content keys. After a takeover the new host must be able to issue a fresh content key, also when the old host held the only copy of the newest one. No record does that today except a removal.
8. Telling the old host, and the members it admitted after the takeover's base, none of whom ordinary sync reaches. What they see, and what a host that was only asleep can do next (it stays a member; it may be handed the seat back).
9. An end and a takeover: an end at or before the takeover's base stands and a takeover based at or after it is excluded; an end after the base is void. Give the rule and who enforces it.
10. What does not move with the seat: outstanding invitations, an open door, the public page's address, the farm service's records. What the takeover's plan tells the new host, and what a public goal's members and visitors see.
11. The recordings of the roles plan's Phases 8 and 9 and automatic admission across a takeover: which key signs after it, and what happens to a stream the old host recorded into after the base.
12. A backup who came through a public door, named by the host's explicit command: the same rule as any backup, or any extra limit.
13. The second and third change of host, and how the new host names its own backup.
14. Versions and release: this piece takes protocol 8 after the first public release and ends the goals made before it. Say what the texts tell people.
15. What is modelled under research/tla before anything is built, and what stays for a later step (a threshold among several named people) and the slot it will use.
Use the fewest phases that each land on a clean tree.`

phase('Design')
const draft = await agent(`${COMMON}\n\n${TASK}`, { label: 'design:backup-host', phase: 'Design', schema: PLAN_SCHEMA, model: 'opus' })

phase('Attack')
const head = d => `${COMMON}\n\nThe design below is for the first version of replacing a Locust host: one optional backup host. The designer's task was:\n${TASK}\n\nTHE DESIGN (JSON):\n${JSON.stringify(d, null, 1)}\n\n`
const SAFETY = `You are an adversary who controls delivery order, who is asleep, who is restored from an old copy, and every hostile member the fault model allows (members, including people who came through a public door, may be hostile; the host and a backup the host named are honest but may be lost, asleep or restored). Produce concrete numbered sequences of events and deliveries that: (1) leave two hosts treated as in force by honest daemons that then never converge; (2) undo a takeover a daemon treated as in force in any case other than the one the owner accepted (the host's removal or un-naming of the backup surfacing later), or undo more than the design says; (3) strand members, their work or their keys, or leave survivors unable to write text; (4) leave the goal stuck for good although the people the design says are enough are available; (5) let a hostile member take, move or block the seat, re-enter after removal, or read content sealed after their removal; (6) make an outcome depend on a clock, on arrival order or on observing that someone is absent; (7) break under a restored copy of the old host, of the backup before and after its takeover, and of an ordinary member; (8) break at the meeting points with ending a goal, the restore guard, the one-member rule, the first-files rule and the automatic recordings. Give a severity for each break. Then list what you tried to break and could not. When unsure, call it a break and say what would settle it. End with the smallest changes that make the design sound.`
const FIT = `Check the design against the code and the product. (1) For each code claim and touch point, open the file and confirm or refute with path and line; say which constraints of the research note's section 2 the design missed. (2) Size: which crates and files change, which signed formats, how many lines by your own judgement, and what must be modelled first. (3) Fit with the three pieces it builds on (KEY, GUARD, END) and the seam report: does it use their plugs as they are, redefine anything they own, or need something they do not give? (4) Product: read only the user explanation, the terminal texts and the commands; predict what happens when a host was only asleep, when the backup is removed, when a backup takes over a public goal; where your prediction differs from the design, that is a break. Count what a host types to name a backup and what a backup types to take over. Name every prompt, refusal or warning that could go without losing safety, and any decision asked of a person that a typical user cannot make. (5) Does it honour every owner decision, including that naming a backup is optional and nothing asks for it, and no migration? Report problems as breaks with severity; put what you confirmed under holds.`
const [attack, fit] = await parallel([
  () => agent(head(draft) + SAFETY, { label: 'attack:backup-host', phase: 'Attack', schema: CRITIQUE_SCHEMA, model: 'opus' }),
  () => agent(head(draft) + FIT, { label: 'fit:backup-host', phase: 'Attack', schema: CRITIQUE_SCHEMA, model: 'opus' }),
])

phase('Revise')
const revised = await agent(`${COMMON}

You wrote the design below for the first version of replacing a host. Two reviewers attacked it. Revise it.

YOUR TASK WAS:
${TASK}

YOUR DESIGN (JSON):
${JSON.stringify(draft, null, 1)}

THE SAFETY ADVERSARY'S REVIEW (JSON):
${JSON.stringify(attack, null, 1)}

THE CODE AND PRODUCT REVIEW (JSON):
${JSON.stringify(fit, null, 1)}

For every break: check the sequence against your rules and the code; if it holds, change the design so it no longer does, or state it in the phases as a limit with the exact case and why it is accepted under the owner's fault model; if it does not hold, say why in one sentence. Prefer the smallest change. Do not add a vote, a quorum or a clock. Keep the owner's decisions. Return the whole revised design in the same shape, and under "answered_breaks" one line per break saying what changed.`, { label: 'revise:backup-host', phase: 'Revise', schema: PLAN_SCHEMA, model: 'opus' })

phase('Verify')
const final = revised || draft
const verify = await agent(head(final) + `This is a REVISION. An earlier draft was attacked; the breaks found then, and the designer's answer to each, are under "answered_breaks". First check each answer: is the break really closed, by the rule as now written, or only renamed? Then attack the revision afresh.\n\n` + SAFETY, { label: 'verify:backup-host', phase: 'Verify', schema: CRITIQUE_SCHEMA, model: 'opus' })

return { draft, attack, fit, final, verify }
