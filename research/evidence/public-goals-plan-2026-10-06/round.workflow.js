export const meta = {
  name: 'write-the-public-goals-plan',
  description: 'Rewrite the public-goals plan: one outline, three phase writers, two checkers, then each part revised',
  phases: [
    { title: 'Outline', detail: 'one settled list of decisions and one phase outline that the writers share' },
    { title: 'Write', detail: 'three writers, each a group of phases, described by behaviour' },
    { title: 'Check', detail: 'one reader on agreement with the other plans and the owner, one on fit with the code and the old plan' },
    { title: 'Revise', detail: 'each part revised against both checks' },
  ],
}

const REPO = '.'
const SCRATCH = 'scratch'

const COMMON = `
You are writing the public-goals plan for Locust v2. Locust is a Rust system in which several people's coding agents work on one goal; each person runs a daemon, and every member's computer keeps a full signed copy of the goal. The repository is at ${REPO}. You are READ-ONLY there: do not edit, create or delete any file in the repository, and do not run cargo, npm or any build or test. Read files and use read-only shell commands (rg, sed -n, wc, git show) only. Another session is building phases 1 to 3 of the roles plan in the code right now; uncommitted changes under crates/, scripts/ and docs/guide/ are theirs.

WHAT PUBLIC GOALS ARE, in the owner's words and decisions: a host can put a goal on a public page and open a door. Strangers' agents join from the page with one command and work. Joining alone gives nothing: their work counts only when a trusted agent approves it. Nothing waits for a human.

READ FIRST
1. ${REPO}/docs/master-plan.md: the opening table, "Decided by the owner" (27 numbered answers; use these numbers), "Assumed until the owner objects", the build order of fifteen phases, and "Versions". Answers 17 to 24 are about public goals. Answer 18 as restated: a task written by someone who came through the door becomes available once a trusted agent approves it (the host's agent, or the agent of a member the host invited); until then no agent takes it; no person is asked. Answer 26: "I don't want the swarm to stop to ask a human for anything." Answer 27: minimize friction and user intervention.
2. The two plans the door is built on, as corrected today: ${REPO}/docs/roles-and-permissions-plan.md (levels, the one check, names, roles, the rules, the shared plan and files landing by themselves) and ${REPO}/docs/host-safety-and-ending-plan.md (K1 the host's key; "What the host's computer signs by itself" with its one rule, whose row 6 is door admission; G1 and G2 the restore guard, with Node::admission_hold; E1 ending; E2 leaving).
3. What exists for public goals: ${REPO}/research/joinable-farms-rewrite-contract-2026-10-05.md (an outline marked "revise before use"), its review ${REPO}/research/joinable-farms-rewrite-contract-review-2026-10-05.md (13 findings, all accepted), the old plan ${REPO}/docs/joinable-farms-plan.md (about half survives) with its mockups under ${REPO}/docs/mockups/joinable-farms, and ${REPO}/research/goal-lifecycle-characterization-2026-10-05.md (measured behaviour of join, leave, remove and the page today).
4. Later inputs, all read-only JSON in ${SCRATCH}: never-wait.json ("audits" has an entry for public goals listing every wait on a person in the contract, with a proposed change for each; "design" is the design for the restated answer 18; "attacks" are two attacks on it, whose breaks must be closed or stated as limits); complete-plan.json (under "roles" and "host", the lists "for_public_goals": what the corrected plans say the door must take from them); leave-round.json ("writer"."skipped" names what the leave design asks of public goals).
5. ${REPO}/research/v2-plan-review-2026-10-06.md, section 5 and cut C8 (the smaller first door), and ${REPO}/research/v2-plan-review-verification-2026-10-06.md, the findings C8, C10 and A2.

DECISIONS FOR THIS REWRITE (the plan author's, under the owner's answers)
a. The plan describes each phase by behaviour and stays true while the fifteen phases before it are built: what works afterwards; what a host, a joiner and their agents see, with proposed terminal and page texts; the signed records and fields and the rule every computer applies; local state; what the farm service does; what it needs from which earlier phase, by that phase's name for it; tests named by the behaviour they show; exit criteria; risks and stated limits. It does NOT list file-by-file changes. Those are written when a phase's turn comes, against the code as it then stands. Say this once at the top. You may name today's files where it helps a reader find what exists.
b. Scope is the smaller first door: one host action that publishes and opens in one plan and one yes (naming a rule change when the goal's rules must change, so a goal made with no flags can be made public without a refusal); one join path from the page; a door that is open or by request as the host chooses (20); the required name shown on the page with no consent step (17); the folding Join band (24); the ended page kept 30 days (23); automatic removal on leave (14, as E2 builds it); the restore guard's gate; a measured ceiling on members.
c. Left out of the first door and named once under "Left for later": the short code and the QR code, extra roster figures, a new brief type and a special fetch order for newcomers (a newcomer's agent reads what every member reads; work must not wait on unrelated content), telling a removed computer that it was removed, everything about replacing a host, and slower dialing for quiet goals.
d. Dropped: refusing a name that another member uses (two members may show the same name; commands tell them apart by a short key), refusing a join for the spelling of an address, the 30-day cap on an open door, and making listed farms admit by request only.
e. A door member's task: the design in never-wait.json, with every break its two attacks found either closed in the text or stated as a limit in plain words.
f. Admission through the door is one of the six things the host's computer signs by itself and must meet that section's rule: signed again from the same records it gives the same record, and it never fails inside start or landing. One validator serves door admission, the host's attended admission and private invitations.
g. After a whole-computer restore or a move, the goals a person hosts wait for one command from that person (the restore guard as decided). The door is closed with a stated reason meanwhile, and the page says so.
h. The door's change of signed bytes: say what changes, and that it stays inside protocol 7 if nothing is released before the door, and otherwise takes 8.
i. Wherever something waits, say on whom. A default that leaves agents' work waiting on a person is not allowed. Admitting by request is the host's own act, like inviting; say plainly what waits there and what does not.
j. Questions for the owner: only what a person using Locust would see or lose, in everyday words, each with a recommendation; at most six for the whole plan.

Write plainly: short declarative sentences, the plans' own terms, no invented labels, no dashes used as punctuation, about 78 columns. Cite the plan or the code (file and line) for claims about what exists today. Say what you verified and what you inferred.
`

const OUTLINE_SCHEMA = {
  type: 'object',
  properties: {
    decisions: { type: 'array', items: { type: 'string' }, description: 'every decision the writers must share, one per entry: records and field names, rule names, command names, words on the page, which earlier phase supplies what; settle each of the review\'s 13 findings here' },
    phases: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'J1, J2 and so on' },
          title: { type: 'string' },
          works_afterwards: { type: 'string' },
          scope: { type: 'string', description: 'what this phase holds and what it must not hold, in a paragraph' },
          needs: { type: 'string' },
          owner: { type: 'string', description: 'which of the three writers writes it: admission, service or lifecycle' },
        },
        required: ['id', 'title', 'works_afterwards', 'scope', 'needs', 'owner'],
      },
    },
    top_markdown: { type: 'string', description: 'the top of the document, ready to place before the phases: title, status, the paragraph on how this plan is written (decision a), "Intended behavior" (what a host is told and what a joiner is told, in everyday words), "Decided by the owner", "What this plan assumes", "What a door member can and cannot do", "Ownership and state" (one table), "Implementation sequence" (one table), and nothing else' },
    left_for_later: { type: 'array', items: { type: 'string' } },
    mockups: { type: 'array', items: { type: 'object', properties: { file: { type: 'string' }, verdict: { type: 'string' } }, required: ['file', 'verdict'] }, description: 'each file under docs/mockups/joinable-farms: still true, or which wording is obsolete' },
    unsure: { type: 'array', items: { type: 'string' } },
  },
  required: ['decisions', 'phases', 'top_markdown', 'left_for_later', 'mockups', 'unsure'],
}

const PART_SCHEMA = {
  type: 'object',
  properties: {
    phases_markdown: { type: 'string', description: 'the phases of this group, each starting with "### Jn: title", in the form: Goal, Depends on, What a person sees, Records and rules, What the computers do, Tests, Exit criteria, Risks and limits' },
    terminal_texts: { type: 'array', items: { type: 'object', properties: { id: { type: 'string' }, title: { type: 'string' }, caption: { type: 'string' }, text: { type: 'string' } }, required: ['id', 'title', 'caption', 'text'] } },
    owner_questions: { type: 'array', items: { type: 'string' } },
    for_the_master_plan: { type: 'array', items: { type: 'string' } },
    answered: { type: 'array', items: { type: 'string' }, description: 'for a revision: each finding and what was done about it' },
    unsure: { type: 'array', items: { type: 'string' } },
  },
  required: ['phases_markdown', 'terminal_texts', 'owner_questions', 'for_the_master_plan', 'unsure'],
}

const TOP_SCHEMA = {
  type: 'object',
  properties: {
    top_markdown: { type: 'string' },
    tail_markdown: { type: 'string', description: 'the end of the document: "## Left for later", "## Questions for the owner" (at most six, merged from all parts, everyday words, each with a recommendation), "## Tests that need people and several computers" (named, and said to be unsized)' },
    answered: { type: 'array', items: { type: 'string' } },
    unsure: { type: 'array', items: { type: 'string' } },
  },
  required: ['top_markdown', 'tail_markdown', 'answered', 'unsure'],
}

const CHECK_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string' },
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          part: { type: 'string', description: 'top, admission, service or lifecycle' },
          where: { type: 'string' },
          problem: { type: 'string' },
          severity: { type: 'string', enum: ['contradicts an owner answer', 'contradicts another plan', 'two parts disagree', 'computers could disagree or a goal could stop', 'work waits on a person', 'wrong about today\'s code', 'missing', 'more than the first door needs', 'wording'] },
          fix: { type: 'string' },
        },
        required: ['part', 'where', 'problem', 'severity', 'fix'],
      },
    },
    holds: { type: 'array', items: { type: 'string' } },
  },
  required: ['verdict', 'findings', 'holds'],
}

phase('Outline')
const outline = await agent(`${COMMON}
YOUR TASK: settle the outline that three writers will share, and write the top of the document.

1. Decisions. Go through the contract, its review's 13 findings, the audit of waits, the door-task design and its attacks, the "for_public_goals" lists and the leave design's notes, and write one list of decisions that leaves the writers nothing to settle between them: the names of the door record and its fields, the field that says how a member came in, the public rule set and what its safety check requires of the rules (including that a door member can never make work count, can never be a goal's only member for the lone-member rule, and cannot share first files), who is trusted to approve a door member's task and how every computer reads it, the one admission validator and its order of tests, what the farm service stores and answers, the publisher's states, the host's one command and the joiner's one command, the door's states and the words the page shows for each, what closes the door and what reopens it, seats and the ceiling, what happens at leave, removal, end, a restore, and with the host's computer off.
2. Phases. Regroup the contract's J0 to J8 into the fewest phases that each leave something working, in build order, for the smaller first door of decision b. Give each to one writer: "admission" (transport limits for callers that are not members, the door record, the validator, how a member came in, the public rules and their safety check, the door member's task), "service" (the farm service, the publisher, the host's command, the page and its Join band, the joiner's command), or "lifecycle" (leaving and seats, ending and the page afterwards, a restore and the door, what a newcomer's agent reads first, the qualification with real agents and several computers). Say what each phase needs from the fifteen phases before it, by their names.
3. The top of the document, ready to place before the phases.
4. Each mockup file under docs/mockups/joinable-farms: still true, or what in it is obsolete.`, { label: 'outline', phase: 'Outline', schema: OUTLINE_SCHEMA })
if (!outline) return { outline: null }

const outlinePack = JSON.stringify({ decisions: outline.decisions, phases: outline.phases, left_for_later: outline.left_for_later })
const GROUPS = ['admission', 'service', 'lifecycle']

phase('Write')
const parts = await parallel(GROUPS.map(g => () => agent(`${COMMON}
YOUR TASK: write the phases the outline below gives to the writer "${g}", and only those. The outline's decisions are fixed: use its names and rules exactly, and if one is wrong or missing, write the phase with the smallest reading and list the problem under "unsure". Follow decision a: behaviour, texts, records and rules, tests by behaviour, exit criteria, risks and limits; no file-by-file change lists. Check every claim about what exists today against the code and the lifecycle note. Give the terminal and page texts your phases need, with ids that start with the phase id.

OUTLINE:
${outlinePack}`, { label: 'write:' + g, phase: 'Write', schema: PART_SCHEMA }).then(p => p && ({ group: g, ...p }))))
const written = parts.filter(Boolean)
if (written.length < GROUPS.length) log('only ' + written.length + ' of ' + GROUPS.length + ' parts were written')

phase('Check')
const docPack = JSON.stringify({ top_markdown: outline.top_markdown, decisions: outline.decisions, phases: outline.phases, parts: written.map(w => ({ group: w.group, phases_markdown: w.phases_markdown, terminal_texts: w.terminal_texts, owner_questions: w.owner_questions, unsure: w.unsure })) })
const checks = (await parallel([
  () => agent(`${COMMON}
YOUR TASK: check a draft of the public-goals plan for AGREEMENT. It is below as JSON: the top of the document, the shared decisions and outline, and three parts. Check it against the owner's 27 answers (most of all 17 to 24, 26 and 27), against the roles plan and the host safety plan as they stand, and part against part. Look for: a door member who can make work count, become the only member, hold a role by joining, or get a task taken that no trusted agent approved (the two attacks in never-wait.json list ways); any default that leaves agents' work waiting on a person; an admission, removal or end that two computers could read differently or that could be signed twice as two different records; something the door needs that no earlier phase supplies; two parts that use different names or rules for one thing; anything from the review's 13 findings, the audit or the "for_public_goals" lists that was dropped; anything in the plan that the smaller first door does not need. Give each finding the part it is in and the smallest fix.

DRAFT:
${docPack}`, { label: 'check:agreement', phase: 'Check', schema: CHECK_SCHEMA }),
  () => agent(`${COMMON}
YOUR TASK: check a draft of the public-goals plan for FIT with what exists. It is below as JSON: the top of the document, the shared decisions and outline, and three parts. Read today's farm service (crates/locust-farm/src), the farm types (crates/locust-proto/src/farm.rs), the publisher (crates/locust-core/src/node/farm.rs), invitations and admission (crates/locust-proto/src/invite.rs, crates/locust-core/src/node/peers.rs, crates/locust-core/src/node/requests/invitations.rs), the sync responder's test for members (crates/locust-core/src/sync/responder.rs), the site (sites/locust.farm/src) and the lifecycle note. Find: claims about today's behaviour that the code contradicts; things described as new that exist today, and things assumed to exist that do not; texts a person would see that do not match the plans' wording for the same thing; a phase that cannot leave something working on its own; steps a host or a joiner must take that could be removed (count the commands and the yeses in each journey from nothing to a first counted result); file-by-file change lists that decision a says to leave out; and where the plan is longer than it needs to be. Give each finding the part it is in and the smallest fix.

DRAFT:
${docPack}`, { label: 'check:fit', phase: 'Check', schema: CHECK_SCHEMA }),
])).filter(Boolean)

phase('Revise')
const findings = checks.flatMap(c => c.findings)
const forPart = p => JSON.stringify(findings.filter(f => f.part === p || f.severity === 'two parts disagree'))
const revised = await parallel([
  () => agent(`${COMMON}
YOUR TASK: revise the top of the public-goals plan and write its end. Below as JSON are the TOP as drafted, the OUTLINE, the three PARTS' questions and notes, and the FINDINGS of two checks that concern the top or say that two parts disagree. Fix what the findings show. Where two parts disagree, state the one rule in the top's decisions so both follow it, and say which you chose under "answered". Then write the end of the document: "## Left for later", "## Questions for the owner" (at most six in all, merged from the parts, only what a person would see or lose, everyday words, each with a recommendation; anything else is the plan author's choice and is stated where it applies), and "## Tests that need people and several computers".

TOP:
${JSON.stringify(outline.top_markdown)}

OUTLINE:
${outlinePack}

PARTS' QUESTIONS AND NOTES:
${JSON.stringify(written.map(w => ({ group: w.group, owner_questions: w.owner_questions, unsure: w.unsure })))}

FINDINGS:
${forPart('top')}`, { label: 'revise:top', phase: 'Revise', schema: TOP_SCHEMA }),
  ...written.map(w => () => agent(`${COMMON}
YOUR TASK: revise one part of the public-goals plan against two checks and return the whole part again in the same form. Below as JSON are the OUTLINE (fixed), your PART as drafted, and the FINDINGS that concern it or say that two parts disagree. For each finding: if it is right, fix the text, preferring to remove over adding; if it is wrong, say why. Where two parts disagree, follow the outline's decision; if the outline is silent, take the reading that asks least of a person and say so. List every finding and what you did under "answered".

OUTLINE:
${outlinePack}

PART:
${JSON.stringify({ phases_markdown: w.phases_markdown, terminal_texts: w.terminal_texts, owner_questions: w.owner_questions })}

FINDINGS:
${forPart(w.group)}`, { label: 'revise:' + w.group, phase: 'Revise', schema: PART_SCHEMA }).then(p => p && ({ group: w.group, ...p }))),
])

return { outline: { decisions: outline.decisions, phases: outline.phases, left_for_later: outline.left_for_later, mockups: outline.mockups, unsure: outline.unsure, top_markdown: outline.top_markdown }, checks, top: revised[0], parts: revised.slice(1).filter(Boolean), drafts_kept: written.length }
