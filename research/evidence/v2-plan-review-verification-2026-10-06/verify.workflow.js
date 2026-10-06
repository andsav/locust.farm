export const meta = {
  name: 'verify-v2-plan-review',
  description: 'Try to refute each finding of the independent v2 plan review against the plans and the code, and scope one recovery option for the restore guard',
  phases: [
    { title: 'Verify', detail: 'six readers, each trying to refute one group of findings' },
    { title: 'Scope', detail: 'one reader scopes recovery by continuing under a new key' },
  ],
}

const REPO = '.'
const REVIEW = REPO + '/research/v2-plan-review-2026-10-06.md'

const COMMON = `
An independent reviewer has reviewed the plan for Locust v2 and concluded: "Do not build the sixteen phases unchanged." Locust is a Rust system in which several people's coding agents work on one goal; every member's computer keeps a full signed copy of the goal as per-author append-only logs. The repository is at ${REPO}. You are READ-ONLY there: do not edit, create or delete any file in the repository, and do not run cargo, npm or any build or test. You may read files and run read-only shell commands (rg, sed -n, wc, git log, git show).

The review is ${REVIEW} (522 lines). It was written against commit 33002ea; the plans have changed a little since (answer 3 was narrowed, answers 18 and 26 were added or restated, see ${REPO}/docs/master-plan.md "Decided by the owner"). Its line numbers refer to 33002ea; use "git show 33002ea:PATH" when a line reference does not match the file as it stands.

The plans: ${REPO}/docs/master-plan.md (read first, short), ${REPO}/docs/roles-and-permissions-plan.md with its companion roles-and-permissions-plan-details.md, ${REPO}/docs/host-safety-and-ending-plan.md with its companion host-safety-and-ending-plan-details.md, ${REPO}/research/joinable-farms-rewrite-contract-2026-10-05.md with its review beside it, ${REPO}/research/v2-complexity-count-2026-10-06.md.

YOUR JOB is to check a group of the review's findings by trying to REFUTE each one. For each finding: read the plan text it cites and the code it cites; rebuild its trace step by step; look for a sentence elsewhere in the plans that already closes it, a code fact that makes its trace impossible, or a misreading. Say "confirmed" only when you rebuilt the trace and could not break it; "partly" when the core holds but a detail is wrong (say which); "refuted" when it does not hold (show why); "cannot tell" when the text or code does not settle it. Do not defer to the reviewer and do not defend the plans. Then, for each finding that holds, state the smallest change to the plans that closes it, where it goes, and whether it is a sentence, an edit to a phase, or a redesign. If a finding leaves a choice that a person using Locust would notice, write it as a plain question in everyday words with what the person would see under each answer; internal choices are the plan author's and are not questions for the owner.

Owner principles that bind any change: a person should meet no friction and protective features are optional and unprompted (answer 1); no migration (answer 2); the swarm never stops to ask a human for anything (answer 26); the failure to design for is a host that disappears, not a hostile host (answer 8).

Write plainly: short declarative sentences, no dashes used as punctuation, cite file and line for every claim.
`

const SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          id: { type: 'string', description: 'the review\'s own label, for example S1, or a short name for an unlabelled claim' },
          claim: { type: 'string', description: 'the finding in one or two plain sentences' },
          verdict: { type: 'string', enum: ['confirmed', 'partly', 'refuted', 'cannot tell'] },
          evidence: { type: 'string', description: 'the trace as you rebuilt it, with plan and code locations' },
          what_is_wrong_in_the_review: { type: 'string', description: 'empty when nothing' },
          changes: {
            type: 'array',
            items: {
              type: 'object',
              properties: {
                document: { type: 'string' },
                where: { type: 'string' },
                change: { type: 'string' },
                size: { type: 'string', enum: ['sentence', 'phase edit', 'redesign'] },
              },
              required: ['document', 'where', 'change', 'size'],
            },
          },
          owner_question: { type: 'string', description: 'a plain question for the owner about what a person would see, or empty' },
        },
        required: ['id', 'claim', 'verdict', 'evidence', 'what_is_wrong_in_the_review', 'changes', 'owner_question'],
      },
    },
    also_found: { type: 'array', items: { type: 'string' }, description: 'problems in the same area that the review did not list' },
    notes: { type: 'string' },
  },
  required: ['findings', 'also_found', 'notes'],
}

const GROUPS = [
  { key: 'guard', prompt: `YOUR GROUP: S1 and S2 (review lines 38 to 109), the two findings that block phase G1, the restore guard. S1 says the marks file is not durable enough for the guarantee it is given: a changed mark is written in place without a sync, a torn mark is discarded as absent, and so a power failure followed by a restore of the database can leave mark and database agreeing at a position the computer has already signed past. S2 says the rules that end a hold by themselves are not proof of recovered history: "heard from every other computer in the goal" reads the member list of the old copy, so computers admitted later are not asked and a stale member proves nothing; and the farm service's answer "Current" proves only the page sequence. S2 also says the planned model leaves the failing case out. Read G1 and G2 in full in the host safety plan (headings "### G1:" and "### G2:"), including its two tables and its "Risks and notes", and today's commit path (crates/locust-core/src/node/commit.rs, crates/locust-store/src/store.rs and connection.rs, crates/locust-core/src/goal/history.rs). Rebuild both traces with named computers and positions. For each release rule in G1's second table say whether it is a proof or a guess, and what evidence would make it a proof. Estimate what one more sync per signed record costs against today's commit (read how SQLite is opened and synced).` },
  { key: 'key-and-agent', prompt: `YOUR GROUP: S4 and S5 (review lines 147 to 199). S4: the key for members and rules does more than the owner's answer says, and because K1 makes a stage task's creator a key that is no member, a custom rule that names the task's creator as the one who must publish, declare, review or attest has no possible actor. S5: K1 keeps the agent a goal was started with unremovable but lets its local credential be revoked for good, after which the goal's first files can never be shared and its page consent can never be given. Read K1 in full (heading "### K1:"), the roles plan's Phase 4 on the first files and the only-member rule, and the code cited (crates/locust-core/src/goal/flow.rs, crates/locust-core/src/node/authoring.rs, crates/locust-core/src/node/farm.rs, crates/locust-core/src/organization/validation.rs).` },
  { key: 'rules-and-local', prompt: `YOUR GROUP: S6, S7, S8 and the paragraphs under "Other readiness boundaries" (review lines 201 to 246 and 297 to 321). S6: the first-files exemption is renewed after every epoch that starts empty, so it is repeatable; and file changes follow the rule printed at workspace init for good, even after the goal's rules change. S7: the "Undo:" line printed after taking a role does not restore the earlier holders. S8: a task allowance is never cleared, so a withdrawn approval revives it. Also: the optional documents setting is absent from two built-in formations; the opinion review under "open" has no matching change to who may review; a combined proposal from both members of a two-member goal can never be approved; and the claim that selector_scope protects a joinable tree from open selectors. Read the roles plan's Phases 3, 4, 8 and 9 and its companion where cited, and the code cited (crates/locust-core/src/goal/rules.rs, fold.rs, crates/locust-core/src/organization/validation.rs).` },
  { key: 'ending-and-versions', prompt: `YOUR GROUP: S9 (review lines 248 to 295) and the version-boundary paragraph (review lines 358 to 364). S9: an end held behind a missing record can leave the public page open while local work is stopped; with a fork of the host's log today's publisher suspends the page, so the complexity count's "the page reads open" is too broad; a fork does not hide the later records from ordinary sync between members; and suspending an ended page keeps its deletion deadline, so the companion's opposite claim is stale. The version paragraph: E1 is called the last change of the event format, yet the public door later adds a signed field to the admission record. Read E1 in full (heading "### E1:") and its companion rows, and the code cited (crates/locust-core/src/node/farm.rs, crates/locust-farm/src/lib.rs, crates/locust-core/src/goal/history.rs, crates/locust-core/src/sync/outbox.rs, initiator.rs, responder.rs, crates/locust-core/src/goal/screen.rs).` },
  { key: 'phase-order', prompt: `YOUR GROUP: section 2 of the review (lines 323 to 371), whether each phase can land in the stated order. Check each row of its table that makes a factual claim: that R1 leaves scripts and executable recipes broken until R6 while CI runs them today (.github/workflows/ci.yml, scripts/check_documentation.py, docs/guide/collaboration.md); that R3 shows each member's latest verdict before R4 makes the latest review count; that R4's model is written only in R7; that R9's check for the word integrator would also match planning documents; that E3 needs neither E1 nor E2; that the master plan's "each piece is modelled before it is built" is not met. For each, say what the plan text says today and whether moving the work to another phase is possible without a cycle. Note for context, and say whether it changes your answer: the owner has said before, of an earlier replacement in this repository, that a few commits breaking the main branch do not matter as long as the work lands clean with no dead code, and asked for judgement over rules; so state the real risk of each broken interval, not the rule.` },
  { key: 'cuts', prompt: `YOUR GROUP: sections 3 and 4 of the review (lines 373 to 453), its ten cuts C1 to C10 and its assessment of the plan author's seven. Check the feasibility claims behind each: C1, that automatic plan recording can be derived whenever a formation names no decider, with no separate documents setting; that deferring R8 would leave the default formation with no current plan and that the public scope uses R8; that today's trial of a new record signs it first (crates/locust-core/src/node/authoring.rs sign_at and crates/locust-core/src/node/commit.rs land_once), so a check before signing needs a new boundary; C4, that the roles plan's Phases 8 to 10 need not wait for the takeover record's shape; C5, C6, C7 and C9 as described; and the six points under "What the count misses or understates". For each cut say whether it is separable as described, what a person would notice, and whether you would take it. Where the reviewer and the plan author disagree (deferring R8 against cutting only its optional setting; the newcomer brief), say which is right and why.` },
]

phase('Verify')
const verifyP = parallel(GROUPS.map(g => () => agent(COMMON + '\n' + g.prompt, { label: 'verify:' + g.key, phase: 'Verify', schema: SCHEMA }).then(r => r && ({ group: g.key, ...r }))))

phase('Scope')
const SCOPE_SCHEMA = {
  type: 'object',
  properties: {
    verdict: { type: 'string', description: 'is it sound in principle, and is it worth a full design round; a short paragraph' },
    how_it_would_work: { type: 'string' },
    what_it_replaces_in_g1: { type: 'array', items: { type: 'string' } },
    what_it_needs_from_the_host_change_design: { type: 'array', items: { type: 'string' }, description: 'the smallest subset of that design, by name' },
    what_stays_unsolved: { type: 'array', items: { type: 'string' } },
    what_a_person_sees: { type: 'string', description: 'after putting back an old copy of their data, as host and as member, compared with the plan today' },
    what_is_lost: { type: 'string', description: 'records dropped, people who must join again, and the like' },
    size: { type: 'string', description: 'rough lines added and removed against the plan as it stands, with the basis' },
    breaks: { type: 'array', items: { type: 'object', properties: { scenario: { type: 'string' }, what_goes_wrong: { type: 'string' } }, required: ['scenario', 'what_goes_wrong'] } },
    owner_decisions_touched: { type: 'array', items: { type: 'string' } },
    alternatives: { type: 'array', items: { type: 'string' }, description: 'other sound ways to recover without proof of history, each in two sentences' },
    unsure: { type: 'array', items: { type: 'string' } },
  },
  required: ['verdict', 'how_it_would_work', 'what_it_replaces_in_g1', 'what_it_needs_from_the_host_change_design', 'what_stays_unsolved', 'what_a_person_sees', 'what_is_lost', 'size', 'breaks', 'owner_decisions_touched', 'alternatives', 'unsure'],
}
const scopeP = agent(`
You are scoping one design option for Locust, a Rust system in which several people's coding agents work on one goal; every member's computer keeps a full signed copy of the goal as per-author append-only logs. The repository is at ${REPO}. You are READ-ONLY there: no edits, no cargo, no npm, no builds. Read files and use read-only shell commands only.

Background. In the v2 plan each goal has one key, kept on the host's computer, that signs who is in and what the rules are. If that key ever signs two different records at one position of its log, membership and rules stop for good. The danger is a computer started from an old copy of its data: it does not know what it signed after the copy was made. Phase G1 of ${REPO}/docs/host-safety-and-ending-plan.md (heading "### G1:") tries to prevent this with a file of marks beside the data folder and with rules for when a held key may sign again. An independent review (${REVIEW}, findings S1 and S2, lines 38 to 109) found that the marks are not durable enough and that the rules which end a hold by themselves ("heard from every other computer", the farm service's answer) are guesses, not proof. For a copy of unknown age no proof exists: the records the computer is missing are the ones that would say who else to ask.

Separately, a design for replacing a host exists as a research note, ${REPO}/research/replacing-a-host-design-2026-10-06.md. Its core is one record, a change of host, at the first position of a NEW key's log, naming a base: the last record of the old key's log that the taker holds. Whatever the old key signed outside the base's ancestry is set aside on every computer, whenever it arrives. The same record lets a host "continue from one branch of its own forked history". That work is planned for after v2.

THE OPTION TO SCOPE: use that mechanism as the recovery for a restore. When the host's computer finds that it was started from a copy and cannot prove what it signed since, it never signs with the old key again. It continues under a new key from the last record it holds. Anything the old key had signed after that record is dropped on every computer when it shows up, instead of becoming a fork. No other computer has to answer first and no person has to run a command. The owner has since said: "I don't want the swarm to stop to ask a human for anything" (answer 26 in ${REPO}/docs/master-plan.md), and had earlier chosen to wait for one command after a whole-computer restore (answer 13), before saying that.

Read G1 and G2 in full, the review's S1 and S2, and in the host-change design note the sections "The design in short", "What the last check found", "The choices" and draft phase B1. Then answer:
- Is the option sound in principle? Build the cases: the restored copy is older than a later admission; older than a removal; the old key's later records reach some members before the new key's first record and others after; the computer is restored twice from two copies; the copy is restored while the original is still running (two computers with the same data); a member's computer, which holds no host key, is restored; a goal the person runs alone.
- What is the smallest part of the host-change design that v2 would have to build for this (which records, which replay rules, which parts of the chain builder), and which of the eight serious breaks its last check found fall inside that part?
- What does it replace in G1 and G2 (the marks, the release rules, the continue command, the catching-up state), and what must stay (for example detecting that the data was replaced)?
- What stays unsolved (a member's own key after a restore; a copy older than the goal)?
- What does a person see, and what is lost (someone admitted after the copy joins again, and so on)?
- Roughly how large is it against the plan as it stands?
- Name other sound ways to recover when history cannot be proved, if any.
Write plainly, cite files and lines, and say what you verified and what you inferred. Default to naming a break when you cannot show that a case holds.`, { label: 'scope:continue-under-a-new-key', phase: 'Scope', schema: SCOPE_SCHEMA })

const verified = (await verifyP).filter(Boolean)
const scope = await scopeP
log('groups verified: ' + verified.length + ' of ' + GROUPS.length + (scope ? ', scope returned' : ', scope MISSING'))
return { verified, scope }
