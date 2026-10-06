# Independent review of the public-goals plan — 6 October 2026

**Verdict: build it after named changes.** Keep the admission provenance, closed
completion rules, trusted-agent task approval, permanent host membership and
single signing gate. Before implementation, close the confirmation race, stop
prompting an early override of a known-incomplete host copy, make the public
creation journey work with an auto agent, fix the approval-discovery rule, and
repair the phase and release contracts below. This is a review of a proposal,
not a qualification of an implementation. The public-goals plan itself says
nothing in it is built ([J:3–20](../docs/joinable-farms-plan.md)).

## Reading boundary and method

I read the master plan first: its opening comparison, all 27 fixed owner answers,
the explicitly provisional assumptions and the build order. Then I read all
5,219 lines of the public-goals plan. Then I followed its dependencies into the
roles and host-safety plans: R1–R6's authority, confirmation, levels, names,
roles and views; R8–R9's document/file acceptance; R7/R10's qualification;
K1's signing contract; G1/G2's holds and continuation; E1's end and E2's leave.
The dependencies being checked are the plan's own lists
([J:2758–2783,3391–3410,3842–3864,4306–4338,4711–4735](../docs/joinable-farms-plan.md)).

The independent code reads used committed files at
`03fe83081056d6e29a0cfc47a819ceaade7d5e75`, through `git show`, not the live
working files another session was changing. I inspected the described paths in
`crates/locust-farm`, proto `farm.rs` and `invite.rs`, core node `farm.rs`,
`peers.rs`, `requests/invitations.rs`, sync `responder.rs`, and the site's farm
page, gallery and client. I followed selected supporting seams in event/replay,
organization validation, sync batching, sessions and CLI confirmation. This was
focused source inspection, not an exhaustive audit of every file in those
directories. Three parallel readers checked safety, code fit and ergonomics.

The independent findings and answers below were written into this note **before
opening** `research/evidence/public-goals-plan-2026-10-06/`. The final section
compares that evidence afterwards. I did not inspect the old PNG mockups,
reproduce historical experiments, run models, start daemons, contact the public
service, or run cargo or npm. The plan says the mockups are obsolete layout
references and its current text governs
([J:5084–5104](../docs/joinable-farms-plan.md)).

References abbreviated **J**, **M**, **R** and **H** mean, respectively,
[joinable-farms-plan.md](../docs/joinable-farms-plan.md),
[master-plan.md](../docs/master-plan.md),
[roles-and-permissions-plan.md](../docs/roles-and-permissions-plan.md) and
[host-safety-and-ending-plan.md](../docs/host-safety-and-ending-plan.md).
Line numbers refer to the reviewed snapshot. A statement about proposed
behavior is a design inference unless identified as a direct source fact.
Named future tests are requirements, not passing test results.

## Findings, most serious first

### 1. A restore prompt can lead directly to a permanent host fork

**High; disclosed safety limit, made easier to trigger by J5.** A public host
with intact marks knows it signed a missing record. J5 nevertheless lists it
under “Waiting for you” after all invited computers, or just one door computer
when none was invited, answer without that record. Another door computer may
still be its sole holder. Continue, sign the next admission/removal/file
acceptance at the reused position, then receive the old record: the host log
forks permanently. J5 states this exact outcome; the release invariant expressly
exempts a person's continuation
([J:4192–4228,4496–4511,4667–4671](../docs/joinable-farms-plan.md)).

This is not a failure of deterministic signing for **the same** request and
records. The restored computer answers a different request or sees different
approved work at an already used position. H explicitly distinguishes these
cases and says only the guard protects them
([H:288–307,340–350](../docs/host-safety-and-ending-plan.md)). Nor is a door
peer less capable of holding the missing record because it is untrusted to
approve work. Admission trust and possession of signed history are different.

**Smallest change:** remove J5's earlier “Waiting for you” promotion for a
known-behind host key; retain G2's all-peer presentation and state plainly that
the mark proves the copy incomplete. Add the trace where the last door peer
returns the missing admission. This closes the newly introduced misleading
prompt, not the inherited manual override. Keep the owner-required command for
an unknown-age restore, and explicitly retain the acknowledged possibility of
a fork after an incorrect override. Absolute no-double-sign safety would need
a different recovery design; the owner has already set that redesign aside
([H:2616–2652](../docs/host-safety-and-ending-plan.md);
[M:125–126,420–431](../docs/master-plan.md)). Do not call invariant 7 a proof
that a host never signs two different records at one position.

### 2. Confirmation does not bind what the daemon actually changes

**High; inferred race in the proposed API.** J4 promises a yes to one rules
transition never applies another. But the plan id stays on the CLI and is sent
nowhere; `farm.door.open` carries no expected rules binding, publication policy
or prior door revision. Rechecking that the current rules are safe does not
check that they are the rules the person approved
([J:2589–2599,2603–2628,3652–3658,3825–3835](../docs/joinable-farms-plan.md)).

For example, after the CLI's final read, a second owner command changes the
goal from peer-review to review-panel. The first command can now open a door
under those safe but different rules instead of performing its approved
peer-review-to-public transition. Likewise, `farm.door.admit { goal, member }`
does not bind the name and endpoint printed in its plan if the waiting entry
changes before handling; those fields are part of the promised confirmation
([J:2531–2538,2591–2592,3593–3597](../docs/joinable-farms-plan.md)).

**Smallest change:** carry an expected digest of the reviewed rules,
publication/disclosure and editable door settings, and compare atomically before
signing. For manual admission carry the reviewed request's fingerprint. Keep
counts and timestamps outside it as intended. Test a change after the CLI's
last read, not only before it re-plans. This is the same kind of explicit
expected-state contract R4 already proposes for role changes
([R:2378–2384](../docs/roles-and-permissions-plan.md)).

### 3. The default create-then-publish journey races its own autonomous agent

**High for the promised first journey; inference.** New agents work at auto,
and a lone default member can open work without another person's step. A
default goal can become public only before any task opens. J4 explicitly
anticipates refusal if a task opens between the plan and the yes. Consequently
an active agent can invalidate the host's next command while the host reads its
long disclosure; the prescribed recovery is another goal
([M:94–96,191–193](../docs/master-plan.md);
[J:268–271,2613–2625,3652–3658](../docs/joinable-farms-plan.md)).

**Smallest change:** document and count creation directly under `public` for
this journey, so tasks opened before publication already have safe rules.
Retain the default peer-review creation for private goals. If the product must
make *default* creation followed by public opening reliable, combine those two
acts into one reviewed create-public command. Do not solve it by asking a human
to stop the agent or change its level. A new public preset is already in J2;
the current budget silently starts after creation
([J:2254–2268,3814–3819,4603–4613](../docs/joinable-farms-plan.md)).

### 4. One rejection blocks discovery even though it does not veto approval

**Medium; hidden operational wait.** One trusted yes is sufficient and another
agent's no is said to block nothing. Nevertheless a latest rejection by **any**
trusted member removes the task from every normal `to_approve` list. Rejected
tasks require deliberate pagination with `--rejected`; tasks shown and left
also disappear from that session until their state changes
([J:288–301,736–739,2417–2427,2700–2716](../docs/joinable-farms-plan.md)).

A first reviewer can therefore prevent the ordinary discovery path from ever
reaching a second willing reviewer. There is no cryptographic veto, but the
unprompted workflow can wait indefinitely. The proposed test actually pins
that behavior ([J:2856–2864](../docs/joinable-farms-plan.md)).

**Smallest change:** retain unapproved tasks in other eligible reviewers'
ordinary discovery, labelled with the rejection; keep per-author fairness and
per-session suppression of that agent's own deliberate decline. State that a
task all agents decline or ignore may remain unstarted forever. Test a second
agent discovering and approving after the first rejects, without a person
knowing to request the rejected page.

### 5. “No human wait” has more conditions than the headline and release gate

**Medium; partly explicit, partly omitted.** The running-session limitation is
honestly stated at the top and in owner question 5. It is not removed by auto,
and the proposed fallback only delivers a local item to an already running
agent. The release gate measures task reviews while a session is running; it
does not guarantee a session continues, a result is reviewed, a lead selects,
or shared files land without a harness approval prompt
([J:74–100,156–177,2737–2751,4887–4893,5136–5143](../docs/joinable-farms-plan.md)).

An additional staffing wait exists in an allowed public `review-panel` goal:
the host alone cannot supply two reviewers, and joining gives newcomers no
role. Someone must invite another reviewer or grant a role. Directed/custom
rules can wait on a particular deciding agent rather than any trusted agent.
These are visible when reading the formations but absent from the top wait
table ([J:272–283,2254–2288](../docs/joinable-farms-plan.md);
[R:2301–2326,2398–2412](../docs/roles-and-permissions-plan.md)).

**Smallest change:** add the omitted waits to the product contract and qualify
the whole default public workflow from task proposal through result review,
plan settlement and file landing with zero post-join human approvals. Say
which supported formations additionally require staffed roles; do not describe
all trusted agents as interchangeable reviewers. Preserve an agent's right to
reject or decline work. Owner question 5 must be resolved before claiming the
default satisfies answer 26, not treated as already accepted
([M:169–178](../docs/master-plan.md)).

The proposed status inference also overreaches: `SessionView.attached` means an
open connection presenting the session secret, not an agent currently thinking
or willing to act. Reconnection alone starts no coding-agent session. Describe
attachment as attachment, and test reconnect-with-no-session instead of saying
reconnect ends the work wait
([J:605–607,1696–1699,2742–2746,4274–4279](../docs/joinable-farms-plan.md);
[api.rs:1753–1767](../crates/locust-proto/src/api.rs)).

### 6. The advertised state order does not produce the advertised refusals

**Medium; observable contradictions.** The following are different conditions,
not merely different wording:

| Conflict | Smallest correction |
| --- | --- |
| A denied request is promised a final answer that stops retries, but denial is checked only at an open door, after closed/full/catching-up; deny is allowed during a hold. A denied join can keep retrying instead ([J:1362–1366,2445–2476,2657–2673](../docs/joinable-farms-plan.md)). | After authentication and credential recognition, let a final denial precede temporary door states. Test denial while full, closed and catching up. |
| Denial is final, but the local denied list holds at most 256 keys and gives no overflow or eviction rule ([J:2531–2542,2671–2675,3704–3709](../docs/joinable-farms-plan.md)). | Define the 257th denial: refuse the host command explicitly, or change the promise if old denials can expire. Never silently evict a supposedly final denial. |
| A full request queue answers `DoorClosed` and tells the joiner only the host can reopen it, while the page says open by request and automatic queue expiry can free an entry ([J:670–671,2535–2541,2995–3000](../docs/joinable-farms-plan.md)). | Give this local refusal a queue-full reason and “Locust will try again”; do not call it a host-closed door. |
| A description expiring after 30 days prints “This door closed”, even though the default door has no closing date and can still be open ([J:316–327,658–678,2688–2691](../docs/joinable-farms-plan.md)). | Say “This join expired; start again from the page.” Handle a page already removed for quiet as unavailable rather than promising that recovery works. |
| The host's end plan permits an operator-defined retention instead of the fixed 30 days, while J3 and owner answer 23 require 30 ([J:1619–1621,3352–3369](../docs/joinable-farms-plan.md); [M:159–161](../docs/master-plan.md)). | Keep shortened periods in test fixtures; remove the product exception or explicitly scope custom services outside that promise. |
| J2's “one line per state” omits ended; the open-or-full quiet example hardcodes open; the mockup commentary uses another quiet sentence ([J:610–640,990–998,5102](../docs/joinable-farms-plan.md)). | Complete the state table and preserve the underlying full/by-request state beneath the service observation. |
| With already-safe rules J4 says the line begins “Rules: public, kept”, although review-panel and directed also pass ([J:1163–1165,2285–2288](../docs/joinable-farms-plan.md)). | Print the actual kept formation and its actual approval/selection requirements. |

The common `door_state` is a good design, but the stronger claim that every
surface reads the same is false for asynchronous service reports, service-clock
expiry, queue-specific refusals and the intentionally uninformed removed
computer. Scope it to the same host snapshot; label the other observations
([J:341–353,3085–3099,3985–3996,4115–4124](../docs/joinable-farms-plan.md)).

### 7. Governance-first does not mean the complete current rules fit frame one

**Medium; source-backed false guarantee.** J1 promises that a joiner has the
first record, members and rules after the first record frame. Sending one
author first cannot do that once the author's log exceeds the batch or byte
limit. Committed batching stops at 256 events or the frame budget; J6's own
churn case creates 2,048 automatic host records, before other governance
records. Formation content is fetched separately
([J:1929–1935,2045–2047,4522–4528,4799–4801](../docs/joinable-farms-plan.md);
[outbox.rs:168–194](../crates/locust-core/src/sync/outbox.rs);
[limits.rs:32](../crates/locust-proto/src/limits.rs)).

**Smallest change:** promise governance-log priority, not complete membership
and rules in one frame. Test a log longer than 256 records and missing formation
content. Keep the newcomer waiting reason and measure admission-to-first-start.
There is no need to invent a new fetch protocol to fix this sentence.

### 8. J2 and J3's exit contracts depend on the future J4 command layer

**Medium; phase independence.** J2 explicitly builds operations and no CLI, yet
promises all printed status commands run. J3's quiet-removal exit requires
`farm door open`, which J4 supplies
([J:2105–2114,2120–2127,3510–3513,3572–3585](../docs/joinable-farms-plan.md)).

**Smallest change:** move the thin host command wrappers and their confirmation
binding into J2; leave page-address joining in J4. Alternatively define J2/J3
as API-only developer slices, use raw operations in their exits, and mark the
CLI texts as future output. J4 already tells a joined agent to read pending
and start; it can deliver that working slice. Moving J5's missing-rules
diagnostic into J4 would improve its first-run explanation, while J5 can still
add history filtering and lifecycle drills
([J:1400–1402,4044–4068](../docs/joinable-farms-plan.md)). This does not require
file-by-file lists.

### 9. Qualification and promotion do not consistently test the released bytes

**Medium; release design.** Both flood and unprompted review are said to precede
the long campaign, but only the 8-member agent run does; a 16-member failure
can still trigger a fallback afterwards. Reducing the ceiling to 8 rebuilds
the candidate and reruns only scale and burst, while the exit contract requires
restore/end/leave at that ceiling and the campaign claims candidate-byte
evidence. Worse, the rollout deploys the incompatible service with a new empty
v2 store before these gates are known, so a failed door release can already
have stopped serving preview pages
([J:3306–3309,4570–4589,4628–4652,4687–4694,4887–4893,4920–4958](../docs/joinable-farms-plan.md)).

**Smallest change:** run both potentially redesigning agent/flood checks before
the long campaign; identify the final candidate and rerun every affected gate
after either fallback or ceiling change. Qualify against a local/staging
service first, then promote service/site before publishing daemons, retaining
the live two-network checks before daemon publication. The service-first
compatibility order remains intact without destroying current pages before
basic qualification.

Also scope the proposed formal invariant to tasks that **need** approval.
“No attempt on a door member's task without approval” omits the expressly
allowed role-holder case; the later invariant and harness test include it
([J:2787–2791,4660–4662,4912–4916](../docs/joinable-farms-plan.md)).

## Soundness answer: what survives the attacks

I found no join-alone route to count work, become the only member, acquire a
role or have a never-approved untrusted task count, **if the named prerequisite
rules land as specified**. These are the concrete reasons and boundaries:

| Attack | Assessment and evidence |
| --- | --- |
| Bring many door identities and approve each other's work | The safety check ignores numeric thresholds and requires a closed selector on every completion path; door admissions carry no role. Normal formation validation rejects zero review counts and empty composites. Sybils can exhaust places, not acquire approval authority ([J:2270–2314,2992–2994](../docs/joinable-farms-plan.md); [validation.rs:125–159](../crates/locust-core/src/organization/validation.rs)). |
| Outlast or remove the host agent and use the lone-member exception | K1 keeps that agent admitted permanently; disconnected is not removed. Only it can match `only_member`, and only its first files get the seed exception ([J:2308–2314](../docs/joinable-farms-plan.md); [R:2322–2346](../docs/roles-and-permissions-plan.md)). |
| Join with a preselected key or an automatic invitation role | Door admission with a role is invalid; the validator refuses directly named keys; later bindings directly naming a current door key are excluded. Public invitees' automatic reviewer role is intentionally a different admission path ([J:2228–2239,2291–2323,2457–2459](../docs/joinable-farms-plan.md)). |
| Use old unsafe rules after a public conversion, or turn the page off | Every door open checks effective task/tree rules; guards persist with current door members after page-off. Existing replay requires root-task rules to be current at its anchor; a new door member cannot simply anchor before its admission. These properties need combined tests, including removal/re-admission ([J:2281–2299,2608–2625,2806–2808](../docs/joinable-farms-plan.md); [fold.rs:282–297,820–832](../crates/locust-core/src/goal/fold.rs)). |
| Start an unapproved task using `allow`, an offer or a subtask | Honest handlers use “available now”; replay holds descendants until an effective trusted approval exists. Level ask cannot override it. Approval of a parent does not approve an untrusted subtask ([J:2348–2387,2389–2427](../docs/joinable-farms-plan.md)). |
| Post before approval or keep using a revoked role on a modified client | Not fully prevented. An early attempt can count after later approval; an old role anchor can remain valid until member removal. Those are expressly admitted limits, not proof that no unapproved computation ever ran. Removal cutoffs, rather than `role take`, enforce the historical boundary ([J:439–444,2957–2975](../docs/joinable-farms-plan.md)). |
| Retransmit an admission or leave | Same-input signing has no clock/random field; existing-member retries sign nothing; E2's removal pins the admission and leave and draws no new content key. Every operation must re-read state before signing. Different restored inputs are finding 1, not covered by idempotence ([J:2440–2506](../docs/joinable-farms-plan.md); [H:3583–3592,3617–3628,3717–3750](../docs/host-safety-and-ending-plan.md)). |
| Interpret admission/removal/end differently on two computers | A common signed host chain and per-record anchors give the same replay from the same records. Partitions legitimately have different knowledge; an end blocks signing on informed computers, not all machines instantaneously, and old-anchor work can arrive later. This is not sealing the board. Actual implementation convergence remains unproved ([J:4126–4156,4669–4671](../docs/joinable-farms-plan.md); [H:2826–2844](../docs/host-safety-and-ending-plan.md)). |

Admission order and expiry use the host's clock/arrival order, but other peers
follow the signed admission rather than independently repeating that choice.
That converges structurally; it is still an unconfirmed interpretation of the
owner's unusually broad answer 3, not the expressly granted exception for
choosing among already-approved changes. Record that interpretation in the
master plan rather than quietly treating it as another fixed owner answer
([M:81–88](../docs/master-plan.md);
[J:328–333,2520–2527](../docs/joinable-farms-plan.md)).

## Every work wait, and who can end it

This inventory separates agent work from entry and publication, which the plan
often mixes. “Explicit” means the top behavior/wait table says it; “buried”
means only phase details or a dependency says it; “inferred” names a consequence.
Ending and a permanent halt are stops, not queues that somebody can approve.

| Work or operation waiting | On whom or what | Visibility and source |
| --- | --- | --- |
| Door-authored task; result/plan/file proposal | An eligible trusted agent's decision, not a human yes | Explicit; a decision can be no or absent forever ([J:164–177,2417–2427](../docs/joinable-farms-plan.md)). |
| Same work with no running trusted session | Someone keeping/starting a coding-agent session | Explicit default limitation, owner question 5; fallback does not launch a session ([J:74–100,2737–2751](../docs/joinable-farms-plan.md)). |
| Only approver at read, disconnected, or sole holder of a deciding role unavailable | That agent's owner restoring its ability/session, or host assigning/inviting a replacement | Explicit status cases; narrower role/staffing wait buried (finding 5; [J:595–607](../docs/joinable-farms-plan.md); [R:2322–2326,2406–2409](../docs/roles-and-permissions-plan.md)). |
| Rejected/left tasks; tasks whose sole approval/role disappeared | Another reviewer discovering and approving them | Withdrawal explicit; discovery suppression buried and can wait indefinitely (finding 4; [J:2389–2415,2700–2716](../docs/joinable-farms-plan.md)). |
| Agent deliberately at ask | Its own person allowing each task, or changing level | Explicit, chosen restriction ([J:177,2409–2415](../docs/joinable-farms-plan.md)). |
| Newcomer's first signature | A further exchange with the host that brings no new records | Buried; busy or sleeping host can hold an already-admitted agent ([J:1705–1709,4250–4252,4517–4521](../docs/joinable-farms-plan.md)). |
| First task while rules/task text are missing | Other computers delivering required content | Explicit for rules; fetch order/content-delay limit buried. Unrelated content should not gate work ([J:1500–1523,4061–4068,4522–4528](../docs/joinable-farms-plan.md)). |
| Approved plan/file change | Host computer online, unheld, with required content; author rebuilds if behind or unlandable | Host wait explicit; rebase/content/private-path blockers in R9 ([J:171,4240–4256](../docs/joinable-farms-plan.md); [R:4450–4471,4522–4531](../docs/roles-and-permissions-plan.md)). |
| Host and its local agents after data restore | Missing signed record from another computer, or host's unsafe override | Explicit; all local agents held with host key, unlike remote agents ([H:1743–1778](../docs/host-safety-and-ending-plan.md); [J:4162–4238](../docs/joinable-farms-plan.md)). |
| Whole-computer restore/move on host | Host's `goal continue` | Explicit accepted exception; hearing from peers does not itself lift it ([J:1671–1677,4171–4173](../docs/joinable-farms-plan.md); [M:125–126,420–431](../docs/master-plan.md)). |
| Restored member's own agent | Host/other computers for the relevant hold, or that member's person continuing | Top table omits this row; J5 admits the omission and adds presentation for marks-kept holds only ([J:4011–4016,4217–4231,4512–4516](../docs/joinable-farms-plan.md)). |
| Voluntary leave, free place and name removal | Leaver stays reachable long enough; host fetch opportunity, removal, then service publication | Host dependency explicit; fetch opportunity inherited from E2. A forked leave requiring manual removal is an exception to the automatic story ([J:1546–1550,4070–4097](../docs/joinable-farms-plan.md); [H:267,3733–3750](../docs/host-safety-and-ending-plan.md)). |
| Open-door join | Host online; then retry timer if previously refused | Explicit host wait; five-to-ten-minute latency buried ([J:166,2677–2695,3010–3012](../docs/joinable-farms-plan.md)). |
| By-request join, host-closed/expired/seat-limited door | Host's person admits/reopens; full goal instead needs removal of a member | Explicit; queue exhaustion masquerades as host closure, and idle Sybils can require host removal (finding 6; [J:167–169,2992–3003](../docs/joinable-farms-plan.md)). |
| Gallery listing; page updates/reappearance | Service operator for listing; host/service for updates; host republishing after quiet deletion or stop | Publication only, not members' work. “Nothing waits” is true only of by-link availability ([J:1112–1129,3235–3275,3338–3366](../docs/joinable-farms-plan.md)). |
| A coding agent's ordinary tool call awaiting its harness approval | That agent's person under its client settings | Not solved by Locust's auto. R7 explicitly says setup does not change approval settings; real-agent gate must record this ([R:4082–4085](../docs/roles-and-permissions-plan.md); [J:1874–1878,3935–3943](../docs/joinable-farms-plan.md)). |

Thus the default public formation removes per-task human approvals only while
appropriately permitted trusted sessions and the necessary computers continue
running. It does not provide an unattended agent service. The first sentence
“No person is asked for anything after the join” needs that scope; the fixed
owner rule must not be silently weakened by an assumed answer to question 5
([J:74–79,156–162](../docs/joinable-farms-plan.md);
[M:171–178](../docs/master-plan.md)).

## Fit to committed code and the fifteen earlier phases

The plan largely distinguishes existing seams from proposed behavior correctly.
It does not mistake today's page for an admission service. Important checked
examples follow; none proves future behavior.

| Existing source fact | Fit and proposed delta |
| --- | --- |
| `plan_join` reads a private ticket first, retries redeemed tickets, checks active host agent and signs with `now_ms`; exchange completion writes two local timestamps ([peers.rs:251–290,302–362](../crates/locust-core/src/node/peers.rs)). | J1's batching of timestamp writes, K1's separate signer and J2's member-first retry order are real changes, correctly identified ([J:1983–1994,2484–2506](../docs/joinable-farms-plan.md)). |
| Same-agent rejoin returns held membership before contacting the host ([invitations.rs:254–276](../crates/locust-core/src/node/requests/invitations.rs)). | The removed-computer false “joined” result is accurately disclosed, not fixed by the door or slower backoff ([J:4115–4124](../docs/joinable-farms-plan.md)). |
| Responder serves a Join before membership, delegates it to the host, and rechecks membership while pumping; `HaltProof` also has a separate path before ordinary nonmember refusal ([responder.rs:98–117,133–177](../crates/locust-core/src/sync/responder.rs)). | Existing transport seam fits J2's single admission validator. J1's blanket “nonmember gets one refusal and nothing more” needs to preserve the bounded halt-proof exception; resource limits must not remove it ([J:1933–1935,1939–1957,2440–2518](../docs/joinable-farms-plan.md)). |
| Policy and signed requests both check `FARM_VERSION`; admission ticket checks the advertised publication policy ([farm.rs:454–465,676–717](../crates/locust-proto/src/farm.rs); [invite.rs:300–310](../crates/locust-proto/src/invite.rs)). | J2's separate policy version before J3's service bump is necessary and correctly ordered ([J:495–522](../docs/joinable-farms-plan.md)). |
| Current page eligibility needs covered members/authors' accepting consent, stops on a disputed latest consent, and publishing sends empty check-ins and retries every failure ([node/farm.rs:108–192,833–855,923–955](../crates/locust-core/src/node/farm.rs)). | Public admission names, pause/stop states, 409 guard evidence and 410 recreation are new work, correctly described ([J:3121–3143,3214–3295](../docs/joinable-farms-plan.md)). |
| Service schema is version 1; no join route; enrollment precheck exempts existing rows/tombstones; old identical receipts return before sequence rejection; retention scans ended pages ([service lib.rs:80–109,283–291,399–422,449–473,814–846,870–917](../crates/locust-farm/src/lib.rs)). | J3's by-link publication, new route, sequence evidence, version 2 and quiet removal are actual changes. Narrow “every unenrolled request gets 403” to new/nonexempt publication: existing receipt replays are deliberately admitted ([J:3300–3389](../docs/joinable-farms-plan.md)). |
| Site envelope types are handwritten; current client uses a stream and initial fetch; page has check-in status and gallery claims every member consented ([client.ts:1–16,28–81](../sites/locust.farm/src/lib/farm/client.ts); [farm page:122–148](../sites/locust.farm/src/routes/farm/[id]/+page.svelte); [gallery:188–200](../sites/locust.farm/src/routes/farms/+page.svelte)). | Join band, polling fallback, generated envelope, new privacy text and copy-to-fold behavior are new ([J:3059–3119,3378–3389](../docs/joinable-farms-plan.md)). |

Two stale provenance statements should be corrected, not treated as architectural
defects: J still says master lists R3 as being built, while M's opening already
says R1–R3 built; M's own review instructions still say only R1/R2 built
([J:16–18,3028–3031](../docs/joinable-farms-plan.md);
[M:3–4,483–488](../docs/master-plan.md)). The two substantive overclaims are
session liveness and first-frame completeness, findings 5 and 7.

| Phase | Can it leave something working? | Earlier-phase naming/dependencies |
| --- | --- | --- |
| J1 | Yes: transport/resource changes independent of the door. Correct the frame-one promise. | “Nothing” is defensible: existing governance identity can be prioritized before K1. Future naming is explicitly distinguished ([J:2016–2022](../docs/joinable-farms-plan.md)). |
| J2 | Yes as an operation-driven two-daemon slice; not with its promised executable CLI text until finding 8 is fixed. Keep the first admission, safety check and task rule together. | Names R1, R3–R6, R8/R9, K1, G1/G2 and E1/E2 correctly. R2 is inherited via R4/R5, not a missing security dependency. J1 is not required for a tiny private fixture, but must precede any public exposure ([J:2093–2114,2758–2783,2937–2953](../docs/joinable-farms-plan.md); [M:344–349](../docs/master-plan.md)). |
| J3 | Yes: publisher/service and read-only band slice. Replace its J4-dependent exits or move wrappers earlier. | R1/R4/R6, K1, G1/G2, E1/E2 and J2 are the right seams. Its `guard_attest` implementation is deliberately deferred here, not evidence it already exists ([J:3265–3283,3391–3410](../docs/joinable-farms-plan.md)). |
| J4 | First actual page-to-membership product slice; its prompt already starts a ready task. Move the missing-rules diagnostic here for a clearer first run. | R2's confirmation grammar, R3 levels, R4 names, R5 status, R6 skill, G2 text and E1 end are correctly named; inherited K1/R8/R9/E2 arrive through J2 ([J:1400–1402,3842–3864,4061–4068](../docs/joinable-farms-plan.md)). |
| J5 | Yes as lifecycle/UX hardening; safety must already hold in J2–J4. | Its R3–R6/K1/G1/G2/E1/E2 list is accurate; changing G2's prompting policy is substantive new behavior, not merely a wording dependency ([J:3998–4004,4306–4338](../docs/joinable-farms-plan.md)). |
| J6 | Qualification and release can stand alone, but it is explicitly allowed to send implementation back to J2. | R6 helper, R7/R10 qualification and G1 restore cases are correctly named; versions remain the master owner's decision. Correct final-byte and promotion order (finding 9; [J:4711–4735](../docs/joinable-farms-plan.md)). |

## Smallest first door and the actual journeys

Keep what fixed owner answers require: open **and** by-request admission, one
required displayed name without separate consent, trusted-agent task approval,
explicit role grants including deciding roles, the folding band, automatic
leave removal, restore guard, end, and 30-day ended page. Invitations, roles
and task execution stay distinct acts
([M:138–178](../docs/master-plan.md)). Suggested reductions concern assumptions:

| Cut, defer or merge | What a person would notice; why it respects the fixed answers |
| --- | --- |
| Defer optional `--seats` and `--expires`; retain a measured concurrent ceiling, lifetime bound and manual close. | Fewer flags, fewer closed reasons and fewer reopening commands. Owners did not require scheduled or admission-budget doors; these are assumed options ([J:307–327,3593](../docs/joinable-farms-plan.md)). |
| Make create-under-public the explicit first journey; consider one create-public plan instead of automatic conversion of unsafe built-ins. | An agent can begin immediately without racing publication. Existing private goals keep their default. The restriction on publishing a private goal with tasks under unsafe rules remains visible ([J:268–283,2608–2625](../docs/joinable-farms-plan.md)). |
| Merge thin door command wrappers into J2 and minimum newcomer guidance into J4. | Every shipped slice's printed commands work; no new user concept or record. This fixes findings 3/8 rather than adding features ([J:2105–2114,3842–3864,3998–4004](../docs/joinable-farms-plan.md)). |
| Start at 8 measured members and defer the 16-member qualification campaign if 8 meets the product need. | Smaller initial farms, an honest stated ceiling, fewer simultaneous sessions and less qualification work. The 16-else-8 campaign is an assumption, not one of the 27 answers; keep meaningful flood, restore, two-network and autonomous-work gates ([M:275–277](../docs/master-plan.md); [J:4577–4589,4868–4893](../docs/joinable-farms-plan.md)). |
| Keep only the latest service receipt after J3 makes older ones unreadable; retain the sequence and deletion tombstone. | No visible loss of latest-request retry or restore detection. Today receipts accumulate forever; J3 itself says nothing reads older ones. Unrestricted by-link publication makes retaining useless rows a poor first-door cost ([J:3327–3336,3371–3376,3544–3546](../docs/joinable-farms-plan.md); [service lib.rs:102–104,870–885](../crates/locust-farm/src/lib.rs)). |
| If there is no separate private release, move/merge the R10 comprehension exercise into J6 so it includes the actual J4 disclosure and J3 band. | One recruited exercise can cover both. Otherwise retain R10 and add a door-specific exercise; the earlier test cannot cover texts that do not yet exist ([J:5216–5219](../docs/joinable-farms-plan.md)). |

I would not cut the task replay rule, the safety check, the host-only first-file
exception, persistent origin, or the restore gate to save implementation effort.
Nor would I add a second “trust this task” human approval; answer 18/26 forbids
that ([J:2291–2314,2348–2415](../docs/joinable-farms-plan.md);
[M:142–147,171–175](../docs/master-plan.md)).

Counts below distinguish human actions from CLI invocations. Reading a page,
copying its prompt and answering a name are not confirmations. Shell installation
details are not completely specified here, so no invented exact setup-command
total is included.

| Journey | Human commands/pastes | Required confirmations | Other input / hidden boundary |
| --- | --- | --- | --- |
| Connected host **starts a goal and makes it public** at a terminal | **2 commands:** `goal create`, then `farm door open` | **2 yeses** | Current one-command budget counts only the second step. No tasks may race in under the default rules; finding 3. Creation's confirmation is in [R:393–400](../docs/roles-and-permissions-plan.md), publication in [J:1169–1203](../docs/joinable-farms-plan.md). |
| Same host through a nonterminal agent/script | At least the two intended operations | **2 human yeses**, **4 CLI invocations** (`--plan`/`--confirm` for each) | A combined prose request can reduce typing, not those two confirmation boundaries ([R:1238–1259](../docs/roles-and-permissions-plan.md); [J:3593,3825–3835,4603](../docs/joinable-farms-plan.md)). |
| Host has not set up Locust, through an agent/script | Installation plus **6 setup/create/publish CLI invocations** | **3 yeses:** setup, creation and publication | Setup adds its plan/confirm pair; shell installation and client setup steps are additional ([R:1467–1478](../docs/roles-and-permissions-plan.md); [J:1385–1388](../docs/joinable-farms-plan.md)). |
| Stranger finds page, Locust/agent already connected, terminal join | **1 command** | **1 yes** | **1 name answer**; at open door no host confirmation. Page discovery/copy is additional UI activity ([J:1257–1284,3814–3819](../docs/joinable-farms-plan.md)). |
| Same stranger through the copied chat prompt | **1 paste**, agent runs **2 join invocations** | **1 yes** | **1 name answer**; status/pending/start are agent calls, not human commands ([J:1372–1405,4604–4605](../docs/joinable-farms-plan.md)). |
| Stranger has not set up Locust | **1 paste**, installation plus at least **4 setup/join CLI invocations** (two plans and two confirmations) | **2 yeses:** setup and join | One public-name answer; client setup may require further interaction. This path is reported but not budgeted by J6 ([J:1385–1399,3819,4609–4613](../docs/joinable-farms-plan.md)). |
| Stranger at a by-request door | Same join journey **plus 1 host admit command** (possibly batched) | **1 additional host yes** | An asynchronous human wait, explicitly chosen by that host ([J:1330–1355,3682–3697](../docs/joinable-farms-plan.md)). |

Starting/sharing actual files adds the host's `workspace init` and one yes; it
is not necessary just to create the public page. Gallery listing adds operator
work, not a host confirmation, and does not delay the by-link page
([R:4472–4486](../docs/roles-and-permissions-plan.md);
[J:3338–3349](../docs/joinable-farms-plan.md)).

## Are the six owner questions the right ones?

They cover real decisions, but they are not six equally open choices. Question
2 mostly restates answer 18. Questions 1 and 6 present false either/or choices.
The text should say whether it is asking about permission to approve **tasks**,
permission to make **results** count, or actual agent availability; “trusted”
alone hides those differences
([M:142–150](../docs/master-plan.md);
[J:272–287,5106–5166](../docs/joinable-farms-plan.md)).

| Current question | Assessment and wording a person can answer |
| --- | --- |
| 1. Any role makes a stranger trusted | Needed, but too broad by implication. Ask: **“Should giving someone any role also let their agent approve other newcomers' tasks, or only roles that explicitly include that permission?”** “Otherwise remove and invite” is not forced: a named approval-bearing role is another design. Explain separately that taking a role away is not a complete revocation against a changed client ([J:5112–5119,2331–2346](../docs/joinable-farms-plan.md)). |
| 2. One yes beats another no | Use as confirmation of the restated task rule, not a new owner veto system. Ask, if needed: **“If one trusted agent accepts a proposed task and another rejects it, should agents still be allowed to take it?”** Fix the discovery contradiction before asking ([J:5120–5124,2700–2709](../docs/joinable-farms-plan.md); [M:142–147](../docs/master-plan.md)). |
| 3. Manually admitted stranger | Worth retaining. Ask: **“When you let someone in from the waiting list, should their agent only contribute work, or also approve other newcomers' tasks?”** Say the recommendation is contribute only. “Trusted like … or like …” makes the owner learn admission terminology first ([J:5125–5130](../docs/joinable-farms-plan.md)). |
| 4. Invitees automatically review | Worth retaining; mention self-approval and formation limits. Ask: **“Should an invited agent be able to make results count immediately, including its own under the default public rules, unless you opt out?”** Not every formation makes one approval sufficient ([J:272–283,2961–2962,5131–5135](../docs/joinable-farms-plan.md)). |
| 5. Nobody running | Essential release decision. Ask: **“For the first release, is it acceptable that newcomers' work waits whenever you and the people you invited have closed all their agent sessions?”** The existing wording is close. A transcript of a running session is not an answer to availability when no session runs ([J:5136–5143,4990–4993](../docs/joinable-farms-plan.md)). |
| 6. Restore reopens an old door | Essential, but “the other choice costs a command after every restore” is too strong. Ask: **“If you restore a backup made before you closed the door, may Locust admit strangers again automatically after recovery?”** Recommend preserving the latest close when available; otherwise show the uncertainty in restore status and in continuation plans where applicable. Marks-kept recovery can be automatic; whole-computer/unknown-age restores already need continuation. Durable/signed settings are another alternative the plan itself lists ([J:4166–4173,5055–5057,5144–5150](../docs/joinable-farms-plan.md)). |

**The missing product question:** **“Must people decide to make a goal public
before an agent opens its first task under the default rules, or should they
be able to open that existing goal to newcomers later?”** The plan chooses the former for the default
rules and forbids conversion after any remote member ever participated. That
changes when people must decide and can force them to start again. It is
explicitly excluded from the six despite not being a fixed owner answer
([J:268–271,2610–2625,5159–5160](../docs/joinable-farms-plan.md)). Treat that
as an owner decision, with the create-under-public option and costs shown.

Admission using the host's clock/arrival order also needs an explicit recorded
interpretation of answer 3; it can be a short clarification of an existing
answer rather than another design quiz. The permanent removal of a quiet page
is a separate non-fixed product cost to disclose beside the fixed 30-day ended
page rule ([J:2520–2527,3352–3369,5161–5164](../docs/joinable-farms-plan.md)).

## Comparison with the plan's own checks

The independent draft was recorded before opening the evidence folder. I then
read its README, the workflow, the outline, both complete check reports,
revision responses and unresolved notes; I used selected embedded draft text
to trace disagreements. I did not reread every superseded embedded draft,
rerun the workflow or inspect its unavailable scratch inputs. The evidence
itself says nothing was built or run
([evidence README:6–15](evidence/public-goals-plan-2026-10-06/README.md)).

**What this evidence is.** There are two checks, each with 34 findings, against
the first draft. They are followed by parallel revisions of the top and three
parts, then a return of the resulting text; the recorded workflow has no final
cross-check of the assembled revision. The revision writers expressly report
not seeing some other parts. Consequently an `answered` entry is evidence of
an author's intended fix, not proof that the final plan agrees with itself
([workflow:138–184](evidence/public-goals-plan-2026-10-06/round.workflow.js);
[round.json:204,465,729,753,1092,1203](evidence/public-goals-plan-2026-10-06/round.json)).

**Where the checks agree with this review:**

| Subject | Agreement and its limit |
| --- | --- |
| Authority and convergence | The agreement check found no joining-only route to counting work, roles or singleton authority, and no same-record-set disagreement. It distinguishes replay from honest start checks, as this review does. These are source/plan readings, not executed adversarial results ([round.json:446–453](evidence/public-goals-plan-2026-10-06/round.json)). |
| Human availability | The check found the running-session dependency, and the revised top explicitly says the no-human-wait instruction is not fully met; a no to owner question 5 blocks this scope. That supports finding 5, rather than resolving it ([round.json:221–232,730,751](evidence/public-goals-plan-2026-10-06/round.json)). |
| Journey counts | The fit check already counted a newly created host goal as two commands/two yeses, three with setup, and the connected stranger as one command or paste, one name answer and one yes. The final budget still begins at an existing goal. This review adds the CLI invocation distinction ([round.json:465,720](evidence/public-goals-plan-2026-10-06/round.json); [J:4603–4613](../docs/joinable-farms-plan.md)). |
| Improvements that reached the plan | The revision removes the default renewal date, accepts an unenrolled listing request by link, makes opening one operation, separates the policy version, and moves the page's join action to J4. Those fixes are present and are not findings here ([round.json:731,734–735,740,1047,1070](evidence/public-goals-plan-2026-10-06/round.json); [J:316–327,495–522,2622–2628,3338–3349,3580–3585](../docs/joinable-farms-plan.md)). |

**Where I disagree with a conclusion or proposed fix:**

1. The restore checker identified the permanent-fork trace, then proposed
   waiting for just one door peer. The lifecycle writer accepted that and
   openly retained the risk of another peer having the missing record. I
   disagree that this is a sufficient correction to the prompt: admission
   trust does not determine who holds signed history. Finding 1 removes this
   early prompting rule while preserving the owner's manual override
   ([round.json:207–218,1199,1208](evidence/public-goals-plan-2026-10-06/round.json)).
2. Rechecking safe rules at operation handling was treated as the fix to the
   two-request problem. It fixes atomicity of the transition, but not whether
   the transition is the one confirmed. The reviewed rules need to reach the
   daemon as expected state; finding 2 is the remaining race
   ([round.json:503–507,734,876,891](evidence/public-goals-plan-2026-10-06/round.json)).
3. The fit check says only Join is served before membership. `HaltProof` is
   another explicit path. I verified that this was also present at its cited
   `3196be8`, not only at this review's later source commit. Its service
   summary correctly limits the enrollment refusal to a **new** page id;
   the final plan's “every request” dropped that qualification
   ([round.json:707,712](evidence/public-goals-plan-2026-10-06/round.json);
   [responder.rs:101–117](../crates/locust-core/src/sync/responder.rs);
   [J:3344–3347](../docs/joinable-farms-plan.md)).
4. The revision calls `SessionView.attached` knowledge of a running session,
   and suggests reconnect as ending the disconnected wait. The field proves
   a secret-presenting connection, not an active agent; reconnect does not
   launch one. The source boundary in finding 5 is narrower
   ([round.json:873](evidence/public-goals-plan-2026-10-06/round.json);
   [api.rs:1753–1767](../crates/locust-proto/src/api.rs)).

**Known to the earlier review, but still incomplete in the final text:** the
full queue's false closed sentence, the 257th denial, unnecessary receipt
retention, and the absence of a door-text reading exercise were all explicitly
flagged as unresolved. They are not new discoveries here. The admission
writer even chose `JoinPending` for an unlisted overflow request, while the
top chose `DoorClosed`; the final plan kept the latter without its needed
reason text. The task-approval invariant was corrected in J6 but left for
J2's writer, and its overbroad J2 sentence survives
([round.json:754,760–765,874,925,1219](evidence/public-goals-plan-2026-10-06/round.json);
[J:2535–2542,2787–2791,3371–3376,5216–5219](../docs/joinable-farms-plan.md)).

**What the recorded checks missed, or stopped short of testing in thought:**

- The after-last-read confirmation race for rules and waiting-request identity
  (finding 2); the checks stop at one operation plus a CLI plan id.
- The autonomous agent opening a default-rules task between creation and
  publication (finding 3). The earlier check caught a scripted run that opened
  work too early, but merely reordered the script; it did not consider an
  independently running auto agent
  ([round.json:524–528,743,1223](evidence/public-goals-plan-2026-10-06/round.json)).
- A rejection suppressing discovery by all other reviewers (finding 4), and
  review-panel staffing or deciding-role absence as additional waits (finding
  5). “One no blocks nothing” is insufficient without the discovery rule.
- Final denial occurring after temporary door states; description expiry
  misreported as door closure; the kept-formation and omitted-ended texts
  (finding 6).
- The bounded-frame counterexample (finding 7), executable host commands in
  J2's promises and J3's exits (finding 8), and final-candidate qualification
  after fallback/rebuild with promotion ordered safely (finding 9). The fit
  check's phase-independence conclusion was limited to J3's visible join
  button, which was fixed; it did not catch these remaining command exits
  ([round.json:551–556,719,1070](evidence/public-goals-plan-2026-10-06/round.json)).
- The need to ask the owner when an existing default goal can become public.
  Six questions were a workflow constraint, and the revision explicitly kept
  the outline's six without adding the service part's concerns. No recorded
  user comprehension test establishes that these are the right six
  ([workflow:37,158](evidence/public-goals-plan-2026-10-06/round.workflow.js);
  [round.json:747,765](evidence/public-goals-plan-2026-10-06/round.json)).

These are remaining design changes and checks to specify before implementation.
Neither this note nor the plan's evidence establishes a working public door.

Review validation: `/opt/homebrew/bin/python3 scripts/check_docs.py` passes on
an isolated staged snapshot containing this note and its index entry. The same
command in the shared working tree fails on the other session's untracked
restore-guard documents linked from `research/README.md` and
`research/tla/README.md`. Those edits were preserved and excluded from this
review's commit. No cargo or npm command was run.
