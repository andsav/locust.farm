# Locust v2 plan review — 6 October 2026

Status: independent source review and recommendations, not accepted design or
runtime qualification. **Do not build the sixteen phases unchanged.** The main
architecture is workable, but the restore guard can automatically release a
signer that will fork, some ordinary actions strand work, and the proposed
phase boundaries knowingly break the repository's existing checks.

## Scope and reading order

I read the master first at `6944de4`, then both phased plans and their companions,
and the public rewrite contract and its review for scope. I formed the cut list
in section 3 before opening the complexity count. I then checked the relevant
source and compared the count. The review snapshot is **`33002ea`**, a descendant
of `6944de4`; production source and the two phased plans are identical at those
two commits. File and line references below refer to that snapshot. The count
comparison uses its corrected version at `33002ea`, not its earlier draft.

E2 is **open**, as requested. Its old manual-removal text is not reviewed as a
finished design. The public contract explicitly says not to assign work from it;
its J labels describe scope, not another ready implementation schedule
([contract:18–26](joinable-farms-rewrite-contract-2026-10-05.md#L18-L26)).
No cargo or npm build or test was run. Source traces below establish what the
current code does and what an implementation of the proposed text would imply;
they do not establish that the proposed implementation passes.

**Concurrent update:** while this review was underway, `0b10413` narrowed owner
decision 3 to permit the host's signed choice, added decision 26 about not waiting
for a human, and reassigned the key-use question to the unattended-signing design.
That resolves the original decision-3 wording conflict identified below. It does
not implement any guard or phase correction. This review retains the fixed
25-decision brief; the later public-task/level questions need reconciliation in
the updated master, not silent substitution into this review
([master at 0b10413:46–54,131–137,265–285](../docs/master-plan.md#L46-L54)).

## 1. Soundness

### S1. G1's marks do not provide the durability its recovery claim needs

**Block G1 on this.** Its commit contract writes a mark before returning and
releasing an event, but its file format overwrites existing marks without syncing
and discards checksum failures. Its startup table lets a copied database sign
immediately when its kept mark is not ahead
([host plan:1241–1246,1308–1337](../docs/host-safety-and-ending-plan.md#L1241-L1246)).

A concrete trace is: keep a backup at governance position N; sign N+1 and send it
to another computer; lose the unsynced mark update in a power failure; restore
the database backup while keeping the old marks directory. Both database and
mark now say N, so G1 releases the key. A different next action signs a different
N+1. Receiving both records permanently halts governance. A torn mark discarded
as absent has the same missing-evidence problem. The plan acknowledges the crash
window but calls the mark only “one record short”; there is no sync on each
replacement to establish a one-record bound
([host plan:1784–1789](../docs/host-safety-and-ending-plan.md#L1784-L1789)).

This matters at the actual durability boundary: today SQLite commits before the
node releases the transaction, using `synchronous=FULL`; G1 would add a weaker
second store after that boundary
([commit.rs:133–139,219–224](../crates/locust-core/src/node/commit.rs#L133-L139),
[store.rs:150–162](../crates/locust-store/src/store.rs#L150-L162),
[connection.rs:40–41](../crates/locust-store/src/connection.rs#L40-L41)).
The existing fork rule truncates the usable prefix at the reused position
([history.rs:78–85](../crates/locust-core/src/goal/history.rs#L78-L85)).

Make the external high-water evidence durable before exposure, or durably record
an uncertainty state that forces recovery after a failed or incomplete update.
Specify crash points, torn-write handling and directory durability. Measure the
cost before choosing an optimization that invalidates the guard's ordinary
restore guarantee. This is a storage-design repair, not another routine question
for the person.

### S2. “Heard from everyone” and a fresh page sequence are not recovered history

**Block the automatic release rules, especially before public admission.** G1
correctly distinguishes a known missing record from a copy of unknown age, but
then clears an unknown host's hold after exchanges with the computers listed in
that old copy. The plan itself supplies the counterexample: those computers can
all be stale or since removed, while a newly admitted computer holds the later
governance records. The host releases itself and forks without an override
([host plan:1253–1265,1770–1784](../docs/host-safety-and-ending-plan.md#L1253-L1265)).
Conversely, one listed computer that never returns prevents automatic recovery
forever. That is a recoverable hold via `goal continue`, not an already-forked
goal; proceeding on incorrect information can turn it into the latter.

The service hook repeats the inference with a different counter. `Current`
clears `UNHEARD`, although the text explicitly admits that it proves only the
page sequence. Take a backup containing page sequence P and governance N; after
the backup, N+1 reaches a peer but no new service request succeeds. Restore both
local stores. The service can legitimately accept P+1 as new while the host is
still missing N+1. Clearing the hold permits a conflicting N+1
([host plan:1448–1462](../docs/host-safety-and-ending-plan.md#L1448-L1462)).
The public review already warns about exactly this unpublished suffix; the new
guard must consume that warning, not turn it into a release condition
([public review:147–185](joinable-farms-rewrite-contract-review-2026-10-05.md#L147-L185)).

Keep a service response useful as evidence that a copy is **behind**, not as
proof that no unseen signed suffix exists. Known durable marks can prove recovery
to those marks. Recovery without them needs a separately specified evidence or
key-replacement protocol, with its safety and availability trade stated. The
fixed brief does not authorize silently narrowing recovery guarantees or making
private goals consult the service
([master:68–86,109–114](../docs/master-plan.md#L68-L86)).

The proposed model campaign also restricts lost-mark restores to scenarios with
no later admissions. That omits the host counterexample above from the claimed
no-reused-position invariant
([host companion:175](../docs/host-safety-and-ending-plan-details.md#L175)).
Include the failing trace explicitly, alongside the conditions under which the
invariant really holds.

### S3. R8/R9 converge on a signed choice, but the original fixed decision forbids how it is chosen

Two approved siblings X and Y can produce different recordings under different
delivery schedules: host sees X alone and records X; in another run it sees Y
alone and records Y. If it sees both, it compares identifiers. R8 and R9 state
this openly
([roles plan:3574–3581,3672–3676,3990–3995](../docs/roles-and-permissions-plan.md#L3574-L3581)).
It contradicts the original decision 3 in the fixed brief
([master at 33002ea:46–47](../docs/master-plan.md#L46-L47)).

**This is not a demonstrated same-records convergence bug.** Other computers
validate the selected subject, predecessor and pinned evidence; they do not
re-run the host's lowest-id choice. Today's decision verifier already validates
the authority, predecessor and proof, and the workspace parent must match its
predecessor or epoch boundary
([fold.rs:698–788](../crates/locust-core/src/goal/fold.rs#L698-L788)).
Thus different signed transcripts can have different winners without two
computers disagreeing on one transcript.

Against the original decision, this is a design blocker requiring different
ordering/conflict semantics, not a documentation disclaimer. The count's proposed
alternative “record neither when two are present” does not by itself fix it:
the first sibling may already have been recorded before the second arrives
([count:198–208](v2-complexity-count-2026-10-06.md#L198-L208)).
The concurrent `0b10413` owner clarification expressly permits the signed host
choice, so this particular blocker is resolved for that newer scope. Retain the
two-transcript test and explain that concurrent losing proposals need rebuilding.

Latest reviews have a related but sound distinction: unrecorded completion uses
the latest review in each reviewer's log; a recorded outcome uses the evidence
it pinned. A late reject can therefore stop a result counting without retracting
a previously recorded plan or tree. That follows owner decision 6. Do not promise
a global “reject happened first” order across different authors' logs
([master:56–57](../docs/master-plan.md#L56-L57),
[roles plan:2170–2188,3694–3701](../docs/roles-and-permissions-plan.md#L2170-L2188)).

### S4. The governance key's job is broader than decision 9, and K1 changes custom stage rules

Decision 9 says the membership/rules key does nothing else. K1 assigns it stage
steps; R8/R9 add plan/file recording. This is an explicit exception to settle in
the signing contract, not an already accepted consequence of separating keys
([master at 33002ea:70–72,269–272](../docs/master-plan.md#L70-L72),
[host plan:590–606](../docs/host-safety-and-ending-plan.md#L590-L606)).
The newer master assigns that choice to engineering; it still needs one exact
list of permitted bodies and failure consequences.

There is also a concrete functional change: a stage task's creator becomes a
non-member governance key. A custom rule requiring `task_creator` to publish,
declare, review or attest then has no possible actor. K1 acknowledges this but
does not assign a formation-validation diagnostic
([host plan:1067–1075](../docs/host-safety-and-ending-plan.md#L1067-L1075)).
Today the runner is supplied as creator to rule resolution
([goal/flow.rs:39–56](../crates/locust-core/src/goal/flow.rs#L39-L56)).
Reject impossible stage rules when binding them, or define a separate working
creator. Do not accept a formation that quietly opens permanently unfinishable
stage tasks. Changing rules/revising work can repair the configuration; this is
not a permanent governance halt.

### S5. Disconnecting the founding agent can permanently remove ordinary capabilities

K1 keeps the founding agent unremovable, yet permits permanent revocation of its
local credential. First files must still be signed by that exact agent; current
page consent also needs its credential. The host can recover roles by giving
them away, but cannot recover first-file sharing or a missing consent for that
goal. Waiting for a future backup host is the plan's proposed way back
([host plan:3319–3322,3349–3356](../docs/host-safety-and-ending-plan.md#L3319-L3322),
[host companion:90–95](../docs/host-safety-and-ending-plan-details.md#L90-L95)).
The current code confirms the dependencies: signing requires an active local
principal, and publication requires consent from active members and covered work
authors
([authoring.rs:117–132](../crates/locust-core/src/node/authoring.rs#L117-L132),
[node/farm.rs:124–162](../crates/locust-core/src/node/farm.rs#L124-L162)).

This is avoidable product friction under an author assumption, not an owner
requirement. Give the host endpoint and first-file authorization identities of
their own, or specify an explicit replacement of the working agent. If allowing
host-agent removal, also replace R4's reliance on “only the host can be the sole
member”; do not accidentally grant a last remaining door member counting power
([roles plan:2037–2051](../docs/roles-and-permissions-plan.md#L2037-L2051)).
That is a coherent design replacement, not a claim that deleting one guard is
sufficient.

Losing the governance key itself is different: a backup older than goal creation
has no key to recover, and v2 has no takeover. Membership, rules, ending and
automatic recording can be lost for good. That is an explicit first-release
availability boundary under decision 22, not grounds to require every person to
configure a backup host
([host plan:1799–1807](../docs/host-safety-and-ending-plan.md#L1799-L1807),
[master:77–80,111–114](../docs/master-plan.md#L77-L80)).

### S6. Two meanings of “first” and “the rule” need correction

Owner decision 16 exempts the first files and requires every later change to
follow the goal's rule. R4 instead renews the exemption after every epoch that
starts empty, and R9 explicitly allows another unapproved seed after resetting
the tree. This is a repeatable exception, not a one-time setup action
([master:94–95](../docs/master-plan.md#L94-L95),
[roles companion:308–313](../docs/roles-and-permissions-plan-details.md#L308-L313),
[roles plan:4022–4024](../docs/roles-and-permissions-plan.md#L4022-L4024)).
Make exemption usage part of the goal's replayed history across epochs. It need
not be a new signed body. Resetting the workspace must not renew it.

R9 also says later file changes follow the rule printed at `workspace init`,
even after `rules bind`; no normal command in this plan starts the new epoch
needed to change it
([roles plan:4017–4021](../docs/roles-and-permissions-plan.md#L4017-L4021)).
Pinned historical evidence is necessary; permanently pinning the rule for all
future file work is a different product choice. Have a confirmed rules change
state the file consequence and, when appropriate, atomically carry the accepted
head into a new epoch. Preserve old evidence without forcing the person through
raw API repair or suggesting that changing the goal's rules changed its tree.

### S7. The promised role undo does not undo the role change

Start with an ordinary role held only by B. `role take B` gives it to the host A
by fallback. The printed `Undo: role give B` then leaves A and B, although the
command and companion promise to restore the old holders
([roles plan:2324–2335](../docs/roles-and-permissions-plan.md#L2324-L2335),
[roles companion:298](../docs/roles-and-permissions-plan-details.md#L298)).
Use a compare-and-set restoration of the exact previous holder set, with one
ordinary command, or print a truthful action name. Do not classify the existing
line as an undo merely because it gives B something back. Keep immediate role
changes, as decision 7 requires
([master:61–64](../docs/master-plan.md#L61-L64)).

### S8. R3's allowance lasts longer than the person is told

The stated allowance expires on completion or revision. Its actual design never
clears `Allowed { round }`; a withdrawn approval makes the same round takeable
again and silently revives the old allowance
([master:137](../docs/master-plan.md#L137),
[roles plan:1594–1604,1963–1978](../docs/roles-and-permissions-plan.md#L1594-L1604)).
Either expire it once this computer observes completion, or explicitly define it
as permission for the entire round, including resumed work after a rejection.
This is an unresolved author assumption about local authority, not shared-state
divergence. The later door-task check must use the same precise lifetime.

### S9. Ending has a public-state gap, but two suspected code failures are already closed

E1 distinguishes an end in force from a held end behind a gap or fork. Either
stops local signing, but only `State.ended` marks the page ended
([host plan:2238–2259,2355–2372](../docs/host-safety-and-ending-plan.md#L2238-L2259)).
A gap before the end can leave a publishable old state open while local work is
stopped. With a governance fork, today's publisher instead suspends the page:
`eligible` refuses authority conflicts. Thus the count's blanket “page reads
open” is too broad
([node/farm.rs:108–116,820–832](../crates/locust-core/src/node/farm.rs#L108-L116),
[count:290–292](v2-complexity-count-2026-10-06.md#L290-L292)).
Assign the held-end page behavior and the start of its retention deadline before
the first door. Owner decision 23 requires an ended page and 30-day removal;
neither a silently open page nor an unacknowledged suspension defines that
contract
([master:115–117](../docs/master-plan.md#L115-L117)).

Two source checks reduce the uncertainty:

- **Forks do not hide the suffix from ordinary authorized sync.** Inventories
  contain every held point, not just the usable prefix; the frontier digest and
  requests expose differing suffixes, and screening retains governance-author
  events even when they are not effective
  ([history.rs:18–30,130–135](../crates/locust-core/src/goal/history.rs#L18-L30),
  [goal/mod.rs:238–254](../crates/locust-core/src/goal/mod.rs#L238-L254),
  [outbox.rs:101–158](../crates/locust-core/src/sync/outbox.rs#L101-L158),
  [initiator.rs:319–356](../crates/locust-core/src/sync/initiator.rs#L319-L356),
  [screen.rs:34–44](../crates/locust-core/src/goal/screen.rs#L34-L44)).
  This supports propagation of `cut_end` between surviving authorized peers,
  conditional on connectivity. It is not delivery proof to removed computers:
  non-member ordinary frames are refused
  ([responder.rs:113–117](../crates/locust-core/src/sync/responder.rs#L113-L117)).
  E1 should make its future sync test assert the surviving-peer outcome rather
  than leave that outcome ambiguous
  ([host companion:298](../docs/host-safety-and-ending-plan-details.md#L298)).
- **Suspending an already-ended page no longer loses its deletion deadline.**
  The companion's opposite claim is stale. Current `Suspend` preserves
  `closed_at`; expiry scans all rows with that deadline, and repeated ended
  uploads retain the original one
  ([host companion:296](../docs/host-safety-and-ending-plan-details.md#L296),
  [farm service:448–469,948–969](../crates/locust-farm/src/lib.rs#L448-L469)).
  Preserve this behavior. It does not solve a held end for which the service
  has never received an ended snapshot.

An end without a frontier intentionally still permits earlier-anchored work to
arrive and count. This is not a sealed final transcript, and the page must not
promise one
([host plan:2774–2786](../docs/host-safety-and-ending-plan.md#L2774-L2786)).

### Other readiness boundaries

R8's optional `documents` setting is absent from `open` and `pipeline`, leaving
them without automatic current plan text despite the master's stated assumption
that every formation without a decider gets the same plan/file rule
([master:138–143](../docs/master-plan.md#L138-L143),
[roles plan:3553–3563,3684–3693](../docs/roles-and-permissions-plan.md#L3553-L3563)).
Likewise the proposed `open` opinion review has no matching change to R3's
review-eligibility row; today's `may_review` returns false outside review rules
([roles plan:1634](../docs/roles-and-permissions-plan.md#L1634),
[rules.rs:250–261](../crates/locust-core/src/goal/rules.rs#L250-L261)).
Resolve these as one product rule, not another settings layer.

A combined proposal sourced from both members of a two-member goal can never
meet an exclude-authors review rule. Rebuilding one's own change avoids that
particular dead end; the plan correctly distinguishes the two. This is a
proposal-level liveness limit, not a broken consensus algorithm
([roles plan:3996–4005](../docs/roles-and-permissions-plan.md#L3996-L4005)).

R9 still claims `selector_scope` protects a joinable tree from open selectors.
The code checks whether task/contribution-relative selectors make sense in their
scope; it does not reject `Members` for being public. The future door needs its
own authority check
([roles plan:4031–4035](../docs/roles-and-permissions-plan.md#L4031-L4035),
[validation.rs:50–75](../crates/locust-core/src/organization/validation.rs#L50-L75)).

## 2. Can each phase land cleanly in the stated order?

**Not as written.** The order is mostly topologically plausible after its open
decisions are resolved, but “no release yet” does not make a failing intermediate
commit clean. R1 explicitly leaves scripts and executable recipes broken until
R6. CI already runs those recipes and the Python helpers, and the guide's marked
recipe uses the removed `--manage-goals` interface
([roles plan:1013–1027](../docs/roles-and-permissions-plan.md#L1013-L1027),
[ci.yml:41–48](../.github/workflows/ci.yml#L41-L48),
[check_documentation.py:53–54,66–72](../scripts/check_documentation.py#L53-L54),
[collaboration guide:93–104](../docs/guide/collaboration.md#L93-L104)).

Here “conditional” means a plausible commit boundary, not an unrun build result.
The sequence and dependency claims are from
[master:200–215](../docs/master-plan.md#L200-L215).

| Phase | Landing assessment and necessary change |
| --- | --- |
| R1 | **No as scoped.** Update executable recipes, affected helpers, their tests and generated contracts with the removed interfaces. Do not defer that work to R6; see the explicit breakage above. |
| R2 | **Conditional.** Bring parser/skill/recipe changes with the grammar, and bind confirmation to material effects rather than every movement of the governance head ([roles:1079–1107](../docs/roles-and-permissions-plan.md#L1079-L1107)). |
| R3 | **Conditional.** Share rule evaluation rather than add a second semantic table; settle the allowance lifetime. Its latest-verdict display precedes latest-review counting in R4, so move those display semantics to R4 or make R3 truthful about its temporary behavior ([roles:1612–1636,1983–1986](../docs/roles-and-permissions-plan.md#L1612-L1636)). |
| K1 | **Blocked on the signing contract in the fixed brief.** The source seams are otherwise explicit, including the two-argument `next_place` until G2. Fix impossible stage rules and the stranded-agent path before calling it complete ([host:672–718,1067–1075](../docs/host-safety-and-ending-plan.md#L672-L718)). |
| R4 | **Conditional.** Fix S6/S7; update every signed-format mirror in this commit. Move the role model here from R7 if retaining “model before code”; the latest-review and only-member cases are explicitly outside that model today ([roles:3402–3458](../docs/roles-and-permissions-plan.md#L3402-L3458)). |
| R5 | **Plausible after R3/R4.** It owns presentation of their actual state; later guard/end additions extend it. Its existing dependency position is sensible ([master:205](../docs/master-plan.md#L205)). |
| R6 | **Plausible as prose consolidation.** It cannot be the first repair of executable material broken five phases earlier. Its declared scope is all guides, site, skill and scripts ([roles:2991–3009](../docs/roles-and-permissions-plan.md#L2991-L3009)). |
| G1 | **Blocked by S1/S2.** Complete storage-failure semantics and the recovery model before integrating automatic signers. The file-identity note is an explicit gate; Time Machine/Migration Assistant are conditional later measurements, not proof already available ([host:1750–1754](../docs/host-safety-and-ending-plan.md#L1750-L1754)). |
| G2 | **Conditional on G1.** Explain proven recovery, waiting and override separately. The deferred `end_held` wiring is correctly assigned to E1 rather than requiring an E1 symbol early ([host:1444–1447](../docs/host-safety-and-ending-plan.md#L1444-L1447)). |
| E1 | **Conditional.** The record and gate can land before R8/R9. Resolve the held-end public projection and update stale retention claims; include source-backed suffix propagation assertions. Public door adapters remain later scope ([host:2220–2268,2774–2797](../docs/host-safety-and-ending-plan.md#L2220-L2268)). |
| E2 | **Open, not approvable yet.** Its rewrite must cover automatic leave/removal, admission identity, cutoff, content-key rotation, retry and guard/end/halt ordering. Decision 14 supplies the requirement, not those transitions ([master:86–87](../docs/master-plan.md#L86-L87)). |
| E3 | **Independent after G1 and deferrable.** It explicitly needs neither E1 nor E2. Do not make it a release prerequisite merely because it occupies slot 12 ([host:2964–2974](../docs/host-safety-and-ending-plan.md#L2964-L2974)). |
| R7 | **Move early models to implementing phases; merge final campaigns with R10.** The two-computer run and comprehension study are real external work, not cargo tests ([roles:3388–3401,3464–3482](../docs/roles-and-permissions-plan.md#L3388-L3401)). |
| R8 | **Blocked under the original decision 3; that wording blocker is resolved by 0b10413.** Still settle automatic plan policy and remove the dependency on the future takeover record's shape ([roles:3540–3559](../docs/roles-and-permissions-plan.md#L3540-L3559)). |
| R9 | **Conditional on corrected R8, G1 and S6.** The explicit R8 infrastructure dependency is real. Its global no-integrator grep also matches companion/planning documents that it does not exempt; scope that gate to live product code/docs rather than requiring historical prose deletion ([roles:3748–3780,3961–3966](../docs/roles-and-permissions-plan.md#L3748-L3780), [roles companion:475–483](../docs/roles-and-permissions-plan-details.md#L475-L483)). |
| R10 | **Valid final integration gate after the fixes.** Keep its additional file, review and restore cases, and run the final swarm campaign here. Comprehension is already repeated only if the explanation changed; do not pretend the plan always demands two cohorts ([roles:4070–4108](../docs/roles-and-permissions-plan.md#L4070-L4108)). |

There is also a version-boundary contradiction: E1 is called the last event-format
change, but the public scope later appends signed `MemberAdmitted.via`. No
migration is needed; simply freeze the format at the actual release boundary,
or give a later core-to-door release another incompatible format number
([master:224–228](../docs/master-plan.md#L224-L228),
[public contract:231–233](joinable-farms-rewrite-contract-2026-10-05.md#L231-L233)).
Do not prebuild dormant takeover bytes to preserve a ladder number.

The master's “each piece modelled before it is built” is an author assumption,
not one of the fixed 25 decisions. Either adopt a precise invariant-to-phase model
matrix or narrow that promise; R7's delayed role model and R8's unmodelled document
stream do not meet its literal wording
([master:169](../docs/master-plan.md#L169),
[roles:3402–3409,3702–3703](../docs/roles-and-permissions-plan.md#L3402-L3409)).

## 3. Independent cuts, deferrals and merges

This list was formed from the plans before reading the complexity count. These
are recommendations, not owner decisions. Where a replacement requires design
work, I say so rather than counting it as free deletion.

| Change | What the person notices; boundary preserved |
| --- | --- |
| **C1 — Cut the optional automatic-plan setting.** Derive automatic plan recording whenever there is no explicit decider, using the same completion rule as files ([roles:3553–3566,3684–3693](../docs/roles-and-permissions-plan.md#L3553-L3566)). | `open` and `pipeline` get a current plan without a hidden extra setting. Named-decider formations keep their explicit authority. This follows the master's assumption, and avoids a second explanation of what an approved revision means ([master:140–143](../docs/master-plan.md#L140-L143)). |
| **C2 — Merge the final R7/R10 real-agent and human qualification effort.** Move invariant models/tests earlier; recruit against the final explanation. Count actual yes responses, setup and recovery, not just wrapper invocations ([roles:3370–3386,3464–3482,4070–4108](../docs/roles-and-permissions-plan.md#L3370-L3386)). | The same finished workflow is qualified once. Earlier lightweight usability trials remain useful. No product protection is removed; artificial command-budget success stops hiding repeated questions. |
| **C3 — Defer E3.** Keep the current polling cadence until idle cost is measured separately from admission load ([host:2986–3015,3066–3071,3096–3101](../docs/host-safety-and-ending-plan.md#L2986-L3015)). | Some extra idle traffic; no new up-to-one-hour missed-push delay and no new Quiet/Receiving-updates oscillation. No fixed owner decision requires adaptive polling. Keep any transport work actually needed for the first door. |
| **C4 — Cut the future takeover-record prerequisite and intermediate format-freeze ceremony.** Preserve named extension points and test current behavior; freeze actual release bytes once ([roles:3545–3547](../docs/roles-and-permissions-plan.md#L3545-L3547), [master:221–228](../docs/master-plan.md#L221-L228)). | Core work and the first door do not wait for backup-host design. People still receive the required warning that the next signed-format release ends these goals ([master:111–114](../docs/master-plan.md#L111-L114)). |
| **C5 — Replace permanent founding-agent special cases.** Decouple transport identity and first-file authority from a revoked working credential; specify a replacement path ([host companion:18,32–33,90–95](../docs/host-safety-and-ending-plan-details.md#L18)). | Replacing an agent does not strand first files or publication. This is a redesign with some new code, not a quantified line saving; retain explicit membership/role/provenance checks as S5 requires. |
| **C6 — Cut repeated first-file exemptions.** One replay-derived exemption per goal, not per empty epoch ([roles companion:308–310](../docs/roles-and-permissions-plan-details.md#L308-L310)). | First setup remains one share; resetting files does not unexpectedly bypass the normal rule. Preserves decision 16 rather than adding a setting. |
| **C7 — Merge related sharing reviews and cut unrelated confirmation invalidations.** Bind material facts and effects, not the blanket governance head; offer a complete create/publish/open or setup/join plan ([roles:1084–1107](../docs/roles-and-permissions-plan.md#L1084-L1107), [public review:265–335](joinable-farms-rewrite-contract-review-2026-10-05.md#L265-L335)). | One informed yes for the complete action; a harmless concurrent admission does not force another review of an unchanged choice. Keep the chosen name, level and disclosures, and revalidate actual authority/capacity at commit. Never silently change custom rules. |
| **C8 — Trim the first public surface.** Defer aliases, QR codes and extra roster metrics; use one URL and responsive Join panel. Drop name-uniqueness refusal, spelling-only join refusal, the unexplained 30-day door cap and mandatory listed ask-mode ([contract:238–243,506–522,617–629,732–740,768–786](joinable-farms-rewrite-contract-2026-10-05.md#L506-L522)). | A link works; fewer repair commands and renewal chores. Keep required names, identity disambiguation, both host-selected modes, explicit promotions and the folding Join band. The mode and promotion removals correct superseded recommendations under decisions 19/20, not cuts to safety ([master:99–119](../docs/master-plan.md#L99-L119)). |
| **C9 — Scope vocabulary cleanup to current product explanations.** Retire misleading interfaces with their replacement; do not let repository-wide string bans require rewriting research or companion descriptions of removed behavior ([roles:2991–3009,3964–3966](../docs/roles-and-permissions-plan.md#L2991-L3009)). | Clear current language, the same operations, and no delayed functional landing merely to make a historical word vanish. |
| **C10 — Put the first door after a usable core milestone, within the v2 program.** Give it a separate acceptance boundary; defer takeover and removed-member notices with their own later work ([master:5–7,219–228,296–299](../docs/master-plan.md#L5-L7)). | Private collaboration becomes usable sooner; the door then arrives with its own tested onboarding and recovery. Do not call the whole promised v2 complete before the first door. Section 5 defines the smaller door. |

## 4. Comparison with the author's count and seven cuts

The count's useful conclusion is the interaction among local recovery state,
unattended signing and shared replay. I agree with its common signing contract:
every automatic path needs an authority, retry identity, durability order and
guard/halt/end behavior
([count:308–326](v2-complexity-count-2026-10-06.md#L308-L326)).
Make the invariant precise: the same logical retry produces the same bytes only
when its predecessor, anchor, evidence and other signed fields are unchanged.
Existing automatic effects use `at_ms = 0`; keep that determinism when extending
the driver
([node/flow.rs:33–46](../crates/locust-core/src/node/flow.rs#L33-L46)).

The revised count correctly labels line estimates, mixed baselines, duplicate
items and unenumerated state products. I did not reproduce its inventory or
turn its “about 100/1,000 reachable states” into a verified state space. Its
20,600–50,000 net Rust-line range is a planning estimate, not an implementation
budget or a forecast of elapsed time
([count:41–64,94–113,139–155](v2-complexity-count-2026-10-06.md#L41-L64)).

| Author's proposed cut | Assessment |
| --- | --- |
| **Defer R8** ([count:336,346–350](v2-complexity-count-2026-10-06.md#L336)) | **Disagree as the preferred v2 cut.** It removes a visible useful outcome and leaves no current plan under the default formation. R9 still needs the recorder infrastructure, and the public scope explicitly uses R8 for reviewed plan text. Prefer C1 and shared recorder machinery. If R8 is deferred, explicitly cut the public-plan promise and rescope its dependencies too ([roles:3553–3589](../docs/roles-and-permissions-plan.md#L3553-L3589), [contract:404–415,819–824](joinable-farms-rewrite-contract-2026-10-05.md#L404-L415)). |
| **Defer J6** ([count:337](v2-complexity-count-2026-10-06.md#L337)) | **Partly agree.** Defer the new brief type and special fetch scheduler, not useful-work readiness or historical task provenance. Existing status/context can expose current rules, plan, accepted files and a next task without waiting for every unrelated blob. The public review already identifies that all-blobs wait and current-admission provenance as wrong ([contract:826–864](joinable-farms-rewrite-contract-2026-10-05.md#L826-L864), [review:412–443](joinable-farms-rewrite-contract-review-2026-10-05.md#L412-L443)). |
| **Defer E3** ([count:338](v2-complexity-count-2026-10-06.md#L338)) | **Agree; C3.** The user effect is more than network use: deferral also avoids the proposed longer recovery delay and flickering public freshness label ([host:3066–3071,3096–3101](../docs/host-safety-and-ending-plan.md#L3066-L3071)). |
| **Defer short code** ([count:339](v2-complexity-count-2026-10-06.md#L339)) | **Agree; C8.** Also consider deferring QR. Neither is among the owner's 25 decisions; the Join fold is, and stays ([master:118–119](../docs/master-plan.md#L118-L119)). |
| **Cut R3's second rules check** ([count:340,351–356](v2-complexity-count-2026-10-06.md#L340)) | **Agree in direction, conditional in implementation.** Reuse one semantic evaluator with structured reasons. Today's trial is not an unsigned preflight: `sign_at` constructs a signed event first, then `land_once` advances and rolls back before persistence. The proposed “never signed” condition therefore requires a new unsigned candidate/evaluation boundary, not simply calling today's trial earlier ([authoring.rs:71–87](../crates/locust-core/src/node/authoring.rs#L71-L87), [commit.rs:197–224](../crates/locust-core/src/node/commit.rs#L197-L224)). Keep local levels, content availability and private-path checks separate from replay validity. |
| **Cut E2's host-action leave line** ([count:341](v2-complexity-count-2026-10-06.md#L341)) | **Agree with removing the manual-host-action requirement; E2 remains open.** The owner still needs truthful “leave sent; removal pending” status while the host is offline or held. Automatic removal is more than deleting a line: its new key and signed cutoff must be committed/retried safely ([master:86–87](../docs/master-plan.md#L86-L87), [count:245–250](v2-complexity-count-2026-10-06.md#L245-L250)). |
| **Cut `Check.count` and `Check.exclude_author`** ([count:342](v2-complexity-count-2026-10-06.md#L342)) | **Agree for this release.** Keep one matching passing attestation under the current check semantics. These are generalization work, not needed to deliver default peer review or the only-member exception. Custom formations would lose the proposed threshold/exclusion feature, so “nothing” is true only for the built-ins ([roles:2077–2085,2130–2142](../docs/roles-and-permissions-plan.md#L2077-L2085), [fold.rs:898–909](../crates/locust-core/src/goal/fold.rs#L898-L909)). |

What the count misses or understates:

1. **Clean intermediate commits:** the explicit R1-to-R6 broken-script interval
   conflicts with CI, not merely with future qualification. Section 2 gives the
   executable evidence.
2. **The mark's failure semantics:** it retains the marks design without resolving
   unsynced/torn updates. Its common durability contract must include S1, not
   merely list that a write happens
   ([count:318–320,358–363](v2-complexity-count-2026-10-06.md#L318-L320)).
3. **A service false release is independent of the abandoned-member stall.** Its
   first gap emphasizes waiting forever. S2 also permits automatically signing
   from an incomplete log after a successful service response
   ([count:240–244](v2-complexity-count-2026-10-06.md#L240-L244)).
4. **Local authority/product contradictions:** the inaccurate role undo,
   resurrected allowance, renewed first-file exemption and inaccessible tree-rule
   change are concrete costs, not captured by the command total. See S6–S8.
5. **Several stale claims have source answers now.** Governance suffixes are
   exchanged, suspended ended pages retain their deadline, and R8/R9 now explicitly
   name the governance key. The count acknowledges only a partial update of the
   latter; R4's member-signed interim acceptance is deliberate until R9
   ([count:264–286](v2-complexity-count-2026-10-06.md#L264-L286),
   [roles:3582–3590,3761–3770](../docs/roles-and-permissions-plan.md#L3582-L3590),
   [host:602–606](../docs/host-safety-and-ending-plan.md#L602-L606)).
6. **Do not turn automatic reviewed recording into an extra permission ceremony.**
   Under the fixed brief, owner decision 18 requires a yes before a door-authored
   task; it does not require another yes before an already approved change is
   recorded. Decision 15 expressly makes that automatic. Applying to a local
   folder and running code are distinct operations and need explicit local
   contracts, but the count's hypothetical extra prompts are not owner decisions
   ([count:213–233](v2-complexity-count-2026-10-06.md#L213-L233),
   [master:91–95,101–103](../docs/master-plan.md#L91-L95),
   [workspace CLI:985–1022](../crates/locust/src/cli/workspace.rs#L985-L1022)).

## 5. Public goals: after the core, still within the promised v2

**The whole public package is too large to hide inside the sixteen-phase release
gate. Build and qualify the private core first; then qualify a smaller first door
as the next v2 milestone.** The master already places the door after R10, and
defines v2 to include it. Moving it out of the product promise altogether would
change scope; adding a core milestone does not
([master:5–7,219–228](../docs/master-plan.md#L5-L7)).
If the core is publicly released before the door changes signed formats, disclose
the incompatible next release; alternatively finish both milestones before the
single public v2 release. No migration or dormant future fields are necessary.

This is a substantial independent workstream: transport/admission budgets,
admission provenance and replay guards, service and publisher lifecycle, joining
and confirmation, ask-mode queues, newcomer context, and physical/network/load
qualification. The contract devotes separate sections to each, including scale
runs and a 300-stream page test
([contract:140–183,192–224,506–526,617–636,730–745,809–824,988–1038](joinable-farms-rewrite-contract-2026-10-05.md#L988-L1038)).
Even its author calls the phase text withdrawn. Treating that as a small tail of
R10 would conceal both design and qualification work.

The minimum first-door scope should contain:

- **One complete host action and one join path:** named disclosure, one chosen
  name, the level required by the fixed brief, a URL, one informed confirmation,
  explicit safe public rules, both open and by-request modes, and the Join fold.
  Fold publish/open and any explicit fresh-goal rule change into one plan; do not
  make the default-rule refusal the normal onboarding path
  ([master:61–64,99–119](../docs/master-plan.md#L61-L64),
  [public review:290–335](joinable-farms-rewrite-contract-review-2026-10-05.md#L290-L335)).
- **Authority and execution checks before the first working admission**, including
  attended admission: original admission provenance for door-authored tasks,
  no automatic counting authority from joining, explicit promotion without
  remove/reinvite, and safe rules for tasks, plan and files. Page-off and
  readmission must not erase provenance
  ([master:101–106](../docs/master-plan.md#L101-L106),
  [public review:32–65,114–145](joinable-farms-rewrite-contract-review-2026-10-05.md#L32-L65)).
  The later decision 26 changes the human-wait question, so that updated contract
  must settle its replacement before exposing the door; it does not make the
  missing check disappear.
- **A working guard and first end, automatic leave, and a consistent page
  lifecycle.** Close admission on end, give waiting joiners a terminal result,
  retain the ended page for 30 days, preserve delete-only `farm off`, and test the
  recovery counterexamples from S1/S2. Backup-host implementation is later by
  owner decision; removed-member notices can remain separate
  ([master:86–87,109–117](../docs/master-plan.md#L86-L87),
  [public review:187–228](joinable-farms-rewrite-contract-review-2026-10-05.md#L187-L228)).
- **Enough context to start useful work and evidence that it works across
  computers.** Reuse current context/status first; do not gate work on every
  unrelated blob. Keep a measured admission/resource ceiling and genuine
  two-computer/network qualification. A simulator, two daemons on one Mac, and
  agents belonging to different people are distinct evidence
  ([public review:412–443,522–529](joinable-farms-rewrite-contract-review-2026-10-05.md#L412-L443),
  [contract:1006–1038](joinable-farms-rewrite-contract-2026-10-05.md#L1006-L1038)).

Aliases, QR codes, extra roster statistics, a new brief schema and adaptive idle
polling can follow. Restore safety, the admission/authority boundary, the end,
the chosen admission mode, the required name and the Join fold cannot be traded
away for that smaller release. Those priorities follow the fixed decisions and
the specific deferrals above, rather than the count's estimated line savings.

## Verification

This review changed only this document and its research index entry. Validation
is documentation/link checking with `python3 scripts/check_docs.py`; no proposed
runtime behavior was built or executed. Findings about future outcomes are
explicit source/design inferences. The two source-resolved concerns in S9 are
not claimed as new runtime measurements.
