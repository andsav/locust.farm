# Joinable farms rewrite contract review — 2026-10-05

Status: independent review findings and recommendations, not accepted decisions
or implemented behavior. Read the [rewrite contract](joinable-farms-rewrite-contract-2026-10-05.md),
the whole [joinable plan](../docs/joinable-farms-plan.md) and
[roles plan](../docs/roles-and-permissions-plan.md), and the research on
[replacing a host](replacing-a-host-2026-10-05.md),
[ending a goal](ending-a-goal-2026-10-05.md) and
[host-key failures](host-key-failure-characterization-2026-10-05.md).
The [roles plan review](roles-and-permissions-plan-review-2026-10-05.md) supplies
the format. The full document reading was at `2209b1b`; while this review was being
written, `2e83592` revised the roles plan. Its relevant only-member and
confirmation changes were checked and are distinguished below. A few current
source paths were also inspected, as identified below. No proposed behavior was
executed and no runtime tests were rerun. The failure note's experiments are its author's
evidence, not new measurements from this review.

**Outcome: revise the contract before using it to assign the rewrite.** Its
owner-answer section settles the priorities, but its executable phase descriptions
still contradict two of those answers. J1 exposes admission before J2 supplies
the safety check, J5 has a second admission path with incomplete checks, and the
first end is bundled with the later backup host. Several remaining refusals and
ceremonies make people repair states the software could safely handle itself.

Below, C means the contract, J0–J8 its phases, RP the roles plan and R1–R10
its phases. JP means the old joinable plan. The owner's answers in C and RP
override all recommendations and stale phase text. In particular, RP's rule is:
“Where two sound designs differ in what they ask of the person, the one that
asks less wins.” A proposed simplification below is not permission to weaken a
guard or silently change the person's choices.

1. **The owner-required check on tasks written through the door has no phase.**

   **Phases: J1, J2, J4, J6; R3 and R5.** C's owner answer for decisions 2, 6
   and 29 says: “a task written by one of them always asks first” and “Level
   auto covers tasks written by the host and by members the host invited,
   never a door member's.” It explicitly includes the host's agent and a
   joiner who chose auto. But C's opening explanation still says “at auto
   tasks that strangers wrote run on that computer unasked”; decision 6 uses
   that behavior to justify a warning, and decision 29 offers auto with that
   same sentence. These are direct contradictions, not open decisions.

   The implementation inherited from R3 is also insufficient:
   “`level_needed`: for `AttemptStarted` and `Resume`, `Ask` when the task's
   allowance is `Allowed` for the task's current round, else `Auto`”. An
   agent already at auto therefore passes without an allowance. J6 adds
   `opened_via` to a view and changes fetching, but changes no start or resume
   check. R5 lists a wanted task only “while its level is below auto”, and
   its refusal fix says “allow this task, or set WHO to auto”. Both must change
   for an auto agent facing a door-authored task.

   Thus the phase text does permit a door member's task to run on another
   computer without that computer's owner being asked, including the host's.
   Changing the warning or putting the task in `ask_first` is not enforcement.
   Nor should giving its author reviewer status, switching the page off, or
   later inviting that author privately silently authorize an existing task.

   **Requested change:** assign one pre-door phase the origin-sensitive start
   and resume check, using the existing round-bound task allowance rather than
   adding a level. Missing provenance must wait, never default to invited. Read origin at the
   task's creation admission, define the rule for subtasks and revised rounds, and update `Abilities`, pending,
   wanted tasks, status and refusal fixes together. Test host and joiner at
   auto, direct starts and resumes, removal/readmission, and `farm off`.
   Preserve auto for host- and privately-invited-member-authored tasks; remove
   every suggestion that raising the level bypasses a door task's required yes.

2. **J1 is an unsafe intermediate phase; the eventual safeguards need an explicit
   invariant across every phase that admits.**

   **Phases: J1, J2, J7; R4, R8 and R9.** J1 says “The host's daemon admits
   many keys unattended” and “Until J4 a door descriptor is redeemed as an
   ordinary ticket with R2's `goal join`”. J2 only then adds “refuses to make
   or keep a goal joinable under rules a door member could meet”. J1's
   dependencies include R4, whose default is `peer-review`. Consequently two
   door identities can approve each other's results in a J1-only build.
   If R9 has already landed, an open tree rule has the corresponding file
   landing exposure. No web page is needed: J1 returns the descriptor.
   This is a development-prefix defect, not a claim that J8 releases without
   J2; J8 does depend on J0–J6.

   After J2 and the relevant roles phases, the intended boundary is mostly
   sound. These are distinct answers to the safety question:

   | Act | Exact contract text and phase | Assessment after the relevant guard exists |
   | --- | --- | --- |
   | Make a result count | J2: “`Reviews { by: role reviewer, count: 1, exclude_author: false }`”; a role is closed because “a door member holds one only after `role give`” | Joining alone gives no counting authority under `public`. An explicit host `role give` intentionally grants it, including approval of the member's own work. That is an owner act, not an unattended promotion. |
   | Change the plan or land files | J2: “a closed completion rule covers the plan text”; it checks “the workspace epoch's binding, each task's effective rules”; R9: “A change lands when it counts under the tree's rule” | No unapproved door change should settle or land. The common statement “a reviewer” applies to `public`; custom safe rules and `directed` need their actual rule described. |
   | Hold a role with one holder | J2: “a role in `Node::deciding(entry)` is refused for a member whose current admission is `via: Door`” | No automatic assignment from a door. The host-command check is consistent with RP's honest-host model; do not present it as a new replay-enforced role-kind rule. |
   | Become backup host | J7: “the chain excludes a naming record or a takeover whose backup's admission at the named base is `via: Door`” | Protected once backup-host support and this replay rule land together. Before then no backup-host mechanism exists. |
   | Run a task elsewhere | C's owner answer: “a task written by one of them always asks first” | Not implemented by the phase contract; finding 1. |

   J2 also narrows the only-member exception only “on a `joinable()` goal”,
   whereas its broader `door_exposed(entry)` explicitly remains true “after
   `farm off` while door members remain”. Whether a door member can become the
   only member depends on the still-unwritten dedicated-key rules for the
   host's working agent. This is an unresolved boundary, not a demonstrated
   sole-member exploit. The exception must not gain a page-off bypass.
   The subsequent RP update adds `Selector::OnlyMember` and says “only a
   host's agent is ever a goal's only member”, based on refusing its removal.
   C's exhaustive selector list must now account for that new variant and
   consume R4's definition, not invent another only-member rule. Preserve
   the non-removable working-member invariant when the dedicated key lands.

   Two other suspected holes have useful current evidence. In
   [rules.rs](../crates/locust-core/src/goal/rules.rs), an unknown task type is
   refused with “task type is not delegated by the definition”; in
   [fold.rs](../crates/locust-core/src/goal/fold.rs), a subtask cannot replace
   its parent's definition and must pass `delegation::narrows`. In
   [chain.rs](../crates/locust-core/src/goal/chain.rs), removal retains only
   the exact cutoff ancestry and later author anchors cannot regress. Merely
   pointing a new approval at an earlier reviewer tenure does not bypass
   that cutoff. These are source findings, not proof of the future R4 rewrite.

   **Requested change:** make the J2 authority guard and finding 1's local
   execution guard prerequisites of the first working admission path, or
   combine those parts with J1. Apply the door-member exceptions independently
   of page visibility, define the sole-member case after separating the host
   key, and retain replay tests for task types, subtask narrowing, old reviewer
   anchors and the host-only first seed. State explicit role promotion as the
   exception to “cannot make anything count”.

3. **Ask-mode admission can bypass the checks on the ordinary door path.**

   **Phases: J1, J2 and J5; later J7.** J1's `plan_join` refuses “a key that
   `participants()` of any binding in `State::rules` names”. J5's separate
   `farm_door_admit` instead lists “an unlapsed entry, a free seat, room under
   the ceiling, J1's gate passed, and a name no current member has taken since
   the request”; it then signs `MemberAdmitted` directly. It does not say to
   repeat the named-key check.

   A concrete sequence: a key enters the waiting list; the host binds otherwise
   safe rules naming that key directly; the key is not yet a door member, so
   J2's current-admission check does not reject that binding; the host admits
   the saved request. J5's listed checks all pass, but J1's promised exclusion
   is broken. The host did deliberately name the key in this example, so this
   is not evidence of a stranger forging authority. It is an inconsistent
   admission contract. J7 likewise adds its named-backup-key refusal only to
   `plan_join`, leaving the shared validation boundary unspecified.

   **Requested change:** give automatic admission and attended admission one
   admission validator. Recheck identity, endpoint, current membership,
   participant references, capacity, recovery and ended state at commit time;
   later add backup eligibility there. Make the deliberate exception that an
   attended admit need not have an open door explicit. Test a rules change
   between queueing and admission, not just a name collision.

4. **The admission gate is not yet the restore guard it claims to reuse, and
   it deadlocks a private goal.**

   **Phases: J1 and J3; prerequisite restore guard.** RP requires recovery
   “until it has recovered its own later records from a member”. J1 substitutes
   “an exchange with a member on another computer has completed”. J3 adds
   “the service has accepted one request as new”. Neither statement proves
   that the governance log is current. C itself acknowledges both limits:
   “a member who came through the door could complete an exchange while
   withholding the host's later records” and the service “shows the page's
   sequence is current, not the host's log”. Decision 11 permits service help;
   it does not declare a new page sequence a signing-safety proof.

   The [failure characterization](host-key-failure-characterization-2026-10-05.md)
   supplies the precise consequence: “None of those conditions distinguishes
   a complete-looking old prefix from a genuinely current one.” Its restored
   invitation experiment signs at a used position before recovery. The
   proposed gate must address that ordering, not just rename it caught up.

   There is also a deterministic liveness problem. J1 gates “a goal this daemon
   hosts, by door or by private invitation” after every start. Its manual exit
   is “the person runs `farm door open` again”. Take an ordinary private goal,
   with only local members, restarted before its first remote admission:
   there is no remote member to exchange with, no farm service to acknowledge
   a request, and opening a door is refused because the goal is not joinable.
   A required backup host would violate the owner's optional-backup answer.

   Finally J1 orders “the gate above; the idempotent answer from the chain”.
   A retry of an already-committed admission needs no new signature and should
   not wait behind the signing gate.

   **Requested change:** let the recovery design own one signing-readiness
   contract and have J1/J3 consume it, with explicit behavior for an old copy
   predating all remote members, a stale or withholding peer, an unpublished
   governance suffix, no reachable member, and a private goal. Keep the
   service's stale-copy detection, but do not claim it closes the unobserved
   suffix window. Distinguish an informed recovery override from proven
   recovery. Return authenticated, already-committed admissions before the
   new-signature gate. Gate automatic plan/file records as well as admissions.

5. **The first end must move out of the backup-host bundle; the version and
   file-acceptance dependencies must follow that order.**

   **Phases: J1–J4, J7 and J8; R8–R10.** C's owner answer for decision 30 is
   exact: “the first door is released after the restore guard and after the
   small first step of ending a goal. The backup host follows later.” Yet J7
   says that until all its prerequisites exist “a public goal cannot be ended”,
   and J8 may run without “`door-end`, `door-removed` and `door-takeover`”.
   The dependency list still places the ending plan only “before J7”.
   Decision 30's recommendation is therefore superseded.

   The [ending note, section 7](ending-a-goal-2026-10-05.md) defines the small
   step: “one host governance record with no frontier”; it stops local signing,
   admission and automatic steps, but “records signed before a computer learned
   of the end still count when they arrive”. It explicitly leaves “Notices to
   removed computers” unbuilt. Requiring the full host notice or a backup host
   before this small step would add a dependency the owner did not request.

   J2 depends on R8 and is merely “Written knowing R9”, yet requires the
   first-files rule “once [it is] written into ... R9”, promises automatic
   landing tests and later edits T1 “once R9 has landed”. That is an unstated
   implementation edge. Separately, decision 9 says “the governance key, the
   end record and the takeover record share a bump”. A later release cannot
   introduce new takeover bytes under an already published format merely to
   keep that promise. Reserving dormant takeover behavior would conflict with
   the repository's current-design-only rule.

   One workable order, preserving RP's order, is: R1–R7 with the decided
   dedicated key and confirmation changes; the restore guard and the takeover
   semantics R8/R9 require to be decided; R8–R10; the small end and its public
   adapters before the first door release; J0–J6 with the authority/start
   checks joined to admission; first-release J8; backup-host implementation
   with the remaining J7 adapters and its qualification later. J0 can land
   earlier. The exact placement of the small end may be earlier too; the
   required edge is before release, not before unrelated groundwork.

   **Requested change:** split J7 by dependency: first-end integration and
   `door-end` qualification are mandatory for the first door; notices and
   takeover integration land with their own implementations. Assign each
   adapter once, make R9 an explicit dependency of the J2 file assertions,
   and publish a version ladder tied to actual release boundaries. Decide
   takeover semantics before R8/R9 without requiring backup-host code then.

6. **Most type extensions have one owner; the page-state seam does not carry
   enough information to implement its promised states.**

   **Phases: J1, J3, J4, J5, J6 and the split J7.** C promises: “Every record,
   type, request, command, endpoint and page element has one owning phase”.
   That should mean one definition with named later extensions, not that no
   later phase can touch it. The following are explicit extensions, not
   duplicate designs: R4 owns `MemberAdmitted.name`, J1 adds `via`; R1 owns
   `GoalJoin`, R3 adds `level`, R4 `name`, J4 `farm`; J1 owns `JoinView`, J4
   adds `address` and `title`; J3 owns `JoinPanel`, J4 supplies `CopyPrompt`,
   J5 adds ask mode; R1/R4's `ContextBrief` becomes J6's `ContextCompact`.
   Keeping one join request, one level writer, R5's `Refused` renderer and
   R2's confirmation implementation is sound.

   But J3 inherits JP's `doorState`, whose first branch is “`closed` (goal not
   open, not accepting, or past expiry)”, before “`full` (`taken >= seats`)”.
   C now requires “a full door uploads no descriptor”, and `FarmDoorView`
   uses “`accepting` in place of `descriptor`”. Full therefore looks closed
   before the Full branch can be reached. J1 also defines Full as “members at
   the ceiling, or `door_admissions >= seats`”, while J3's upload has only
   `seats` and `taken`, not the current member count or an explicit Full
   reason. The site cannot derive capacity-full from those fields.

   J3 can show door states before J4 adds a join action; that is not an order
   defect. The defects are the missing state information and branch precedence.
   Likewise a first-end adapter must extend the same state mapping, not create
   a second meaning of Ended alongside the old goal-scope close.

   **Requested change:** keep the existing field-extension ownership, but give
   J3 one explicit host-reported door-state/reason contract consumed by the
   service and site, with local expiry/offline overlays clearly advisory.
   Test capacity-full, used-seats-full, manual close, catching up, off and end
   separately. Allocate the end adapter to the pre-release end step and remove
   the inherited closed-before-full algorithm.

7. **The join plan lacks its exact confirmation input, so routine page updates
   can turn one yes into repeated planning.**

   **Phase: J4, using R2.** J4 says “the plan id of confirm.rs binds the command
   and its arguments” and “`--confirm` fetches again and fails with `conflict`
   if the plan changed”. R2 actually hashes `{"command","review"}`, not all
   arguments or printed text: “`human`, `warning` and `again` are not hashed”.
   J4 never defines `review`. T3 includes the service's door line, and J3's
   `FarmJoinView` includes `service_time_ms` and `received_at_ms`. Hashing that
   whole view would make confirmations expire as time, occupancy or a check-in
   changes. Hashing too little could omit the chosen name, level or disclosure.

   There is also a literal acquisition mismatch: J4 says “fetches the descriptor
   once”, “fetches once per run”, then “`--confirm` fetches again”; the inherited
   alias route requires alias resolution plus the descriptor request. These
   need a precise unit: one descriptor request per invocation, not one HTTP
   request for the entire human journey.

   **Requested change:** specify J4's `Plan.review` fields and tests. Bind the
   acting identity, signed goal/farm and issuer identity, name, level and
   disclosure being authorized; exclude advisory occupancy, service clocks and
   last-seen text. Decide explicitly which signed descriptor changes require
   another yes. Revalidate live admission separately, reuse R2, and test a busy
   door and a harmless check-in between plan and confirmation.

8. **The proposed journey has more inputs than “one command” suggests, and
   publishing can safely ask less.**

   **Phases: J1, J2 and J4; R2–R4.** J1 requires `farm on --joinable`
   “(plan then confirm, T1)” and `farm door open` “(plan then confirm, T2)”.
   J2 refuses a default `peer-review` goal and names `rules bind --formation
   public`. J4 “asks for the name and the level, then shows the plan (T3) and
   asks to proceed”. These give the following planned counts, not measurements:

   | Starting point and endpoint | Commands the person enters at a terminal | Other typed responses | Total submitted lines |
   | --- | --- | --- | --- |
   | Existing fresh goal already under safe public rules → link-only page with open door | `farm on --goal G --joinable`; `farm door open --goal G --seats 16 --expires 7d` | Two yes responses | 4 |
   | Connected host agent, no goal → same endpoint | `goal create --title T --formation public`, then the two commands above | Three yes responses | 6 |
   | Existing fresh default `peer-review` goal → same endpoint | `rules bind --goal G --formation public`, then the two publication commands | Three yes responses | 6, plus an avoidable failed `farm on` if the host discovers the requirement that way |
   | No goal, host creates with the default then repairs it → same endpoint | Four commands | Four yes responses | 8, before counting a failed attempt |
   | Connected joiner, one eligible local agent → join requested | `locust --owner farm join ADDRESS` | Name, level, yes | 4 |
   | Same joiner, choices supplied as flags | `farm join ADDRESS --name Maple --level ask` with the owner prefix | One yes | 2 |

   These counts exclude installation, work, optional reviewers, files, status
   reads and admission waits. A listed ask-mode farm adds a host admit command
   and yes for each joiner. Taking the first task at ask, or any door-authored
   task at auto after finding 1 is fixed, adds one owner `allow` command; RP's
   owner answer means it adds no confirmation after that command.

   In chat, J4's T6 says “ask me for the name and the level”, then show the plan,
   then “`--confirm ID` after my yes”. For a connected agent that is one pasted
   prompt, one answer containing both choices, and one yes: three owner
   messages if the questions are combined, four if asked separately. The agent
   runs two join invocations, not two commands typed by the person. Fresh setup
   adds the two `up` invocations and another plan/yes round. The updated R2
   explicitly classifies `up` as asking because it changes client files and
   takes a name for good. That disclosure must survive; a combined setup/join
   review could ask once for the complete action.

   A safe fresh goal does not need two separate publication ceremonies. One
   reviewed action can show the public rules, disclosure, seats and expiry,
   then publish and open. Likewise, when a fresh default goal needs `public`,
   the reviewed publishing plan can propose that rules change explicitly.
   It must not silently replace custom rules or expose an existing private
   history. Separate low-level commands may remain for people who want them.

   **Requested change:** add counted terminal and chat journeys to J8 and give
   the ordinary host one combined publish-and-open plan, including an explicit
   safe-rule change when appropriate. Offer creation in that same journey if
   desired, under one complete plan. Gather the joiner's name and level
   together, skip setup when connected, and retain one informed join yes.

9. **Remove the avoidable prompts, refusals and warnings, while keeping the
   ones that enforce a real boundary.**

   **Phases: J1–J7, with R2–R5 owner corrections.** This is the friction audit
   of the specified surfaces. Each quoted item is from C unless RP is named.
   “Remove” means use the safe replacement in the last column; it does not
   mean suppress an error while leaving the unsafe behavior available.

   | Surface and exact text | Unnecessary burden and safe replacement |
   | --- | --- |
   | J1: publish and open each “plan then confirm” | Combine for the ordinary journey, as in finding 8. Show sharing facts once. |
   | J2: “refused at `farm on --joinable`” for default `peer-review` | Offer the explicit safe-rule change inside that publishing plan. Keep rejection when custom or historical rules cannot safely be changed. |
   | J4/T6: “ask me for the name and the level” | One question containing both choices; no additional level confirmation. Do not invent the required public name or default auto. |
   | J4/T6: “set up with ... `up ... --plan`, then ... `--confirm ID`” | Skip setup if already connected. For fresh setup, combine setup and joining into one complete review if practical. The current R2 says setup takes a name permanently and changes client files; include those facts before that yes. |
   | C's inherited R2 skill: “first with `--plan`, then with `--confirm` ... after their yes” | RP `2e83592` has already corrected this: “If not, it applies at once: run it once”. Consume that correction in the rewrite. An owner-authorized level change, allowance or role change needs no second question. |
   | J1: “`NameTaken` when the name equals a current member's” | RP R4 already tests duplicate names with `a_member_resolves_by_key_prefix_then_name_and_a_shared_name_lists_key_prefixes`. Display a key prefix and provenance when ambiguous. Exact-byte uniqueness does not prevent look-alike names, and the owner required a name, not a globally unique one. |
   | J4: “`goal join` with a door descriptor is refused and names `farm join`” | Where the address/disclosure is available, route to the same verified public-join implementation. Refuse missing required disclosure, not the command spelling. A raw ticket without enough information still needs that information. |
   | J4: “fails with `conflict` if the plan changed” | Retain conflicts for material choices; remove conflicts caused only by advisory page churn, finding 7. |
   | J1: “the gate above; the idempotent answer from the chain” | Already-admitted retries can return their recorded result without another signing-recovery wait. |
   | J2: a deciding role is refused and “a host who wants that removes the member and invites it privately”; decision 5 repeats that for backup host | An explicit host promotion can carry the same authority decision without eviction, a ticket, another join and lost local allowances. If adopted, preserve original task provenance and still forbid automatic promotion. This changes a recommendation, not an owner answer. |
   | J3/decision 21: “a full door serves none and the page shows no prompt” | Permit an explicitly requested paced wait at a capacity-full door. J1's separate active-member ceiling can clear after removal; used admission seats not returning is not a reason to forbid every wait. Do not promise eventual admission. |
   | J5/decision 22: “A listed farm must use ask mode” | No counting, role, backup or local-execution invariant depends on gallery placement. Keep host-selected ask mode and operator enrollment, but do not force per-person host approval merely for listing without a separate product reason. |
   | J7/decision 18: “No automatic removal” after a signed leave | Under the door's disclosed standing policy, automatically process a voluntary leave of an ordinary door member, applying the existing role fallback and removal/key rules. No host must manually approve a departure the member requested. Handle any separately granted responsibilities explicitly. |
   | Decision 25: door expiry “keep[s] its 30-day cap although invitations have none” | Keep a finite owner-chosen expiry and disclose it, but remove the unexplained extra cap and monthly renewal work. A longer interval explicitly chosen in the plan is not unattended expansion of permission. |
   | J6: “if ... objects are missing, say so and wait” | Wait for the dependencies needed for the chosen work, not every unrelated blob; finding 11. |
   | J1/T1 and T2 both include live-session, always-on-machine, seats, names, IP, old-copy and lost-computer explanations | Keep one concise disclosure for the combined action and move operational detail to status/help. Do not repeat the entire onboarding warning on every seat or expiry edit. |
   | Decisions 6/29: “tasks strangers wrote run here unasked” | Delete this false warning after implementing the owner's rule. Keep the narrower explanation that auto starts host/invited tasks while door tasks require an allowance. |
   | J1: “Never start this computer's Locust data from an old copy while the door is open” | A permanently repeated warning is not a restore guard. Put actionable recovery information on the actual held/recovery state, with the residual risk stated accurately; finding 4. |

   Keep the single sharing confirmation; the whole-history and irreversible-copy
   disclosure; required name and level; identity/signature/version/goal binding
   checks; resource ceilings; explicit task allowance for door-written work;
   material plan-change conflicts; and unsafe-rule refusals with no sound
   automatic repair. Keep ending/removal confirmations because the owner
   explicitly requires them. Deleting a page spends its address, so its
   confirmation should not disappear merely because door close applies at
   once. RP `2e83592` now makes invitation revocation immediate with an
   “Invite again:” line; consume that upstream choice rather than adding a
   public-door confirmation back. RP still lists that classification as
   unconfirmed by the owner. Cancelling a still-pending join signs
   nothing and need not inherit a member's leave ceremony. The contract
   already correctly removes close/deny confirmations, separate name consent
   and demands for a backup host. “Backup host: none named” is a fact, not a
   prompt; it need not become a warning or a question.

   **Requested change:** make a command-by-command friction table authoritative
   over inherited generic confirmation prose. Adopt the safe removals above,
   keep the listed boundaries, and measure the resulting inputs rather than
   describing a multi-question journey only as “one command”.

10. **Ask-mode names and denial reversal need a complete state transition.**

    **Phase: J5, reusing R2/R4.** J5 advertises
    “`farm door admit --member KEY|PREFIX|NAME`” and depends on
    `resolve_member`. RP R2 resolves from “the `goal.status` members”, and
    R4 adds matching a member's name. Waiting requests are not members.
    A new candidate source is needed; redefining `resolve_member` or copying
    its matching algorithm would give the same selector two owners.

    J5 also promises “`farm door deny` applies at once and a later admit of
    that key lifts it”. But it inherits JP's “Deny moves keys from `pending`
    to `denied`”, where denied holds keys only, and requires “an unlapsed
    entry” to admit. A denied key gets terminal `InvitationRefused`; the
    joiner stops. The endpoint/name needed for admission and the mechanism
    that resumes that stopped join are not specified. Merely deleting a key
    from the denied list cannot produce the promised immediate admission.

    **Requested change:** have J5 supply waiting-request candidates to R2's
    shared resolver, with ambiguity behavior for waiting names and key
    prefixes. Define denial reversal end to end: what request data survives,
    what the host command actually does, and how the joiner's consent and
    retry are renewed when needed. Test deny→admit, expiry→admit and two
    waiting requests with the same name; do not promise an inverse that the
    stored state cannot perform.

11. **The newcomer can be blocked by unrelated content, and provenance is
    read at the wrong time.**

    **Phase: J6; decisions 2 and 26.** J6 sets `objects_missing` from
    “`BlobIndex::wanted`” and instructs the agent: “if `catch_up.rules` is
    false or objects are missing, say so and wait”. It also prioritizes all
    “`DocumentRevised` payloads”, while limiting priority task payloads by
    “opener's current admission”. A door member can post revisions and
    name content that has not arrived. Even with usable rules, an accepted
    tree and a ready task, the instruction makes the newcomer wait for that
    unrelated material. Prioritizing trusted task text does not fix a gate
    over all missing content.

    JP's retained deferred section is explicit: “one joiner can make every
    member's daemon download what it names”. Decision 26 is therefore a
    fetch-order choice, not a content-fetch safety boundary. The catch-up
    criterion needs to work under that admitted limitation.

    Current admission is also the wrong provenance for existing text: removing
    a door author and privately readmitting it would reclassify its old tasks.
    Conversely an old invited author's work could acquire a door label after
    readmission. The same distinction matters for finding 1's execution guard.
    Guidance needs the signer of its rules binding, not automatically the
    current host's agent after a takeover.

    **Requested change:** distinguish readiness for the selected work from
    background fetch completion, prioritize the effective plan rather than
    every proposed revision, and read provenance at the authored record's
    admission/anchor. Add a newcomer case with unavailable unrelated blobs and
    a readmission case. State clearly that priority does not cap downloads;
    do not add another owner prompt as a substitute for a correct readiness
    predicate.

12. **End, leave and takeover need precise promises in the first-release text.**

    **Phases: J1, J3, J4 and the split J7.** C's opening says “ending is final”.
    J7 freezes the publisher “after one last upload” and says “after the end
    a name leaves the page only with the whole page”. The small ending step
    instead leaves earlier-signed work able to arrive and count, and the
    [ending note, section 6](ending-a-goal-2026-10-05.md) says of an end after
    a takeover's base: “The end is void like any other late host record”.
    Calling the board sealed or the end unconditionally final would promise
    more than either design provides. The first release has no takeover,
    but its later introduction needs an explicit change to that explanation.

    Name withdrawal before an end already has an owner: J1's `consented` and
    `eligible` show a leave-request author as “former member”. J7's leave
    sentence is a later explanation of that behavior, not a second projection
    implementation. With a frozen ended page, a late leave/decline cannot
    remove the name under decision 15. This should be disclosed at the end,
    with a useful default: taking the page down as part of the already-confirmed
    end saves a subsequent `farm off` and the retained-name problem. Keeping
    the page can remain an explicit choice; the owner has not answered this
    retention decision.

    The [replacement note, section 8](replacing-a-host-2026-10-05.md) still
    requires a decision about “whether the host's later removal of the backup
    outranks the takeover”. J7's check at the named base does not resolve that
    problem. A no-door-backup check alone is not a complete takeover contract.
    Nor is a new page address evidence that an old page is current: C already
    correctly limits descriptor verification to “signed by the key the page
    published, not that this key still hosts the goal”.

    **Requested change:** specify first-end behavior without a frontier, what
    late records can still do, and the page's final-upload/delete behavior in
    the pre-release adapter. Prefer page deletion in that same end action,
    with explicit retention available. Keep name suppression owned by J1 and
    only extend its lifecycle rules. Before backup support, settle revocation,
    stale-base, proof delivery and end-versus-takeover behavior in the
    replacement design, then make J7 consume those rules and update its text.

13. **The decision list must distinguish superseded recommendations from
    remaining choices, and qualification must test the owner answers.**

    **Phases: J1–J8 and the prerequisite list.** C still says “The owner
    decisions below answered before the writers start: 1, 2, 3, 4, 7, 8, 11
    and 32 block J1, J2 and J4; 30 fixes whether J8 waits for J7”. Decisions
    1, 2, 11 and 30 have answers already. Nothing should ask them again.
    The following are my disagreements with the numbered recommendations;
    the last column separates direct contradiction from a different design
    recommendation under the owner's governing rule.

    | Decision; affected phase | Exact recommendation at issue | Assessment |
    | --- | --- | --- |
    | 3; J1/J2 | “refuses the default `peer-review` with a sentence naming `public`” | Keep the preset and guard; disagree with a repair command as the ordinary path. Present the safe change in the publishing plan, finding 8. No direct owner answer on the preset. |
    | 4 and 5; J2/later J7 | “a role that picks or closes, never”; “never, not even by explicit naming”; “removes the member and invites it privately” | Disagree with remove/reinvite as promotion. One explicit host act can confer the same authority with less disruption, while joining alone confers none. These restrictions are recommendations, not recorded owner answers. Keep the existing restrictions until the replacement is specified and tested. |
    | 6; J1/J2 | “since the host's own agent otherwise takes tasks strangers wrote unasked” | Directly contradicts the owner's answer to 2/6/29. Keep host auto; enforce the task-origin allowance. |
    | 7; J4 | “`goal join` refuses a door descriptor and names `farm join`” | Agree on one `goal.join` request; disagree with a spelling-only refusal where the same verified public join can be performed. Missing disclosure remains an error. |
    | 9; release ladder | “the governance key, the end record and the takeover record share a bump” | Disagree as an unconditional requirement across releases. It becomes incompatible with owner decision 30 if it postpones the first release until backup-host bytes exist. |
    | 11; J1/J3 | “the door waits for one request the service accepts as new” | Agree with owner-authorized service help; disagree that this is sufficient proof of recovered signing history. The owner approved help, not this particular sufficiency claim. |
    | 15; first-end adapter | “30-day deletion and no withdrawal after the end” | Prefer end-and-delete-page by default with explicit retention. This saves a second command; the owner has not decided retention. |
    | 18; leave handling | “no; status shows 'asked to leave; still holds a place' with the remove command” | Prefer automatic processing of an ordinary door member's signed voluntary leave under the door policy; no extra host decision is needed for that case. |
    | 21; J3/J4 | “a full door serves none and the page shows no prompt” | Disagree with forbidding a paced capacity wait; distinguish active capacity from spent admission seats. |
    | 22; J3/J5 | “yes; the gallery badge reads 'Open by request'” | Disagree with mandatory ask mode based solely on listing. It provides no additional authority or local-execution protection once the substantive guards hold. |
    | 25; J1 | “keep its 30-day cap although invitations have none” | Disagree absent a concrete distinct safety reason. Keep explicit finite expiry; let the host choose it. |
    | 26; J6 | “only [tasks opened by members who did not come through the door], until the deferred fetch limits exist” | Accept this as priority only. Disagree if treated as safety protection or a reason to wait for all other content; use historical provenance and useful-work readiness. |
    | 29; J4 | “tasks strangers wrote run here unasked” | Direct contradiction of the owner answer. Keep auto as an explicit choice, with the required door-task exception and no later level-setting ceremony. |
    | 30; J7/J8 | “yes ... a public goal cannot be ended” | Direct contradiction. First release waits for the small end; backup support and removed-member notices need not be bundled with it. |

    I have no separate objection to 1, 2, 8, 10, 12, 13, 14, 16, 17, 19,
    20, 23, 24, 27, 28, 31 or 32, subject to the concrete corrections above.
    In particular: keep the required chosen name without separate name
    consent; allow door members to open tasks; keep the restore prerequisite;
    keep local door state; do not silently renew a join beyond its reviewed
    expiry; and do not require a backup host. Decision 28's real-agent run
    may remain recorded without gating subjective speed or review quality,
    but no failure of an authority or owner-allowance invariant may be waived
    as merely a disappointing live run. Decision 32 can gather both choices
    in one exchange rather than two.

    J8's real-agent scenario currently specifies only “at level ask”. That
    cannot establish the owner's special rule for auto. Its “restored-host
    cases pass” also needs the exact recovery cases in finding 4, not merely
    a successful exchange. The missing acceptance matrix includes auto on
    both sides, private invitations after restart, admission races, page-off
    behavior, late content during catch-up, and the first end with an open
    door and waiting joiners. Physical-network, scripted, model-agent and
    current-source evidence must remain separate claims.

    **Requested change:** mark 1, 2, 6, 11, 29 and 30 answered, replace their
    stale recommendations and all dependent wording, and leave only genuinely
    undecided alternatives for the owner. Assign each missing invariant to
    its implementing phase and a mandatory first-release test, then make J8
    report those results and the counted journeys. Qualify backup-host cases
    when backup support actually lands.
