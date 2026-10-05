------------------------------ MODULE Workspace ------------------------------
EXTENDS Naturals, Integers, Sequences, FiniteSets, TLC

(***************************************************************************
Bounded proposed workspace contract. Signed event IDs, signatures, policy hashes,
and goal identities are abstract. A fixed adversarial transcript is delivered in
every order, including duplicate/stutter delivery. No Rust refinement is claimed.

Governance orders/fences epochs without validating workspace proofs. A checkpoint
validates its exact decision/predecessor/provenance/evidence closure while ignoring
only workspace successor disputes. Pins remain local to that proof. A typed
RetainBefore choice restores an exact ancestor epoch's inherited boundary; it is
explicit administrator rollback, never an inference from missing objects/proofs.
***************************************************************************)

CONSTANTS Scenario, CheckCheckpoint, CheckFence, CheckNullBoundary, CheckSources,
          CheckExcludeAuthors, ScopeIsolation, CheckPinnedRules, CheckCutoff,
          CheckExactEvidence, RequireContent

E(id, author, slot, prev, kind, round, subject, parent, prior, evidence,
  sources, expected, checkpoint, restore, rules, goal) ==
 [id |-> id, author |-> author, slot |-> slot, prev |-> prev, kind |-> kind,
  round |-> round, subject |-> subject, parent |-> parent, prior |-> prior,
  evidence |-> evidence, sources |-> sources, expected |-> expected,
  checkpoint |-> checkpoint, restore |-> restore, rules |-> rules, goal |-> goal]
G == E(1,0,0,0,"rules",0,0,0,0,{}, {},0,0,0,1,1)
Epoch(id,slot,prev,expected,checkpoint,restore,rules) ==
 E(id,0,slot,prev,"epoch",0,0,0,0,{}, {},expected,checkpoint,restore,rules,1)
P(id,author,slot,round,parent,sources) ==
 E(id,author,slot,0,"proposal",round,0,parent,0,{},sources,0,0,0,1,1)
R(id,author,slot,round,subject) ==
 E(id,author,slot,0,"review",round,subject,0,0,{}, {},0,0,0,1,1)
S(id,author,slot,round,subject,prior,evidence) ==
 E(id,author,slot,0,"select",round,subject,0,prior,evidence,{},0,0,0,1,1)
InitialEpoch == Epoch(2,1,1,0,
 IF Scenario = "invalid-initial" THEN 3 ELSE IF Scenario = "missing-initial" THEN 99 ELSE 0,0,1)
Seed == <<P(3,1,0,2,0,{}),R(4,2,0,2,3),R(5,3,0,2,3),S(6,4,0,2,3,0,{4,5})>>
Handoff == Epoch(11,2,2,2,6,0,1)
NewWork(round,parent) == <<P(12,1,1,round,parent,{}),R(13,2,1,round,12),
 R(14,3,1,round,12),S(15,4,1,round,12,0,{13,14})>>
Fork == S(16,4,0,2,3,0,{4,5})
Restore(id,slot,prev,expected,target) == Epoch(id,slot,prev,expected,0,target,1)
SourceWork == <<P(20,2,1,2,6,{}),P(21,3,1,2,6,{20}),
 P(22,1,1,2,6,{21}),R(23,2,2,2,22),R(24,3,2,2,22),
 S(25,4,1,2,22,6,{23,24})>>
HistoricalWork == <<P(22,2,1,2,6,{}),R(23,1,1,2,22),R(24,3,1,2,22),
 S(25,4,1,2,22,6,{23,24})>>
RulesChanged == E(90,0,2,2,"rules",0,0,0,0,{}, {},0,0,0,90,1)
Removal == E(91,0,2,2,"remove",0,2,0,0,{}, {},0,0,0,1,1)
AdminFork == Epoch(92,1,1,0,0,0,1)
ForeignSource == E(20,2,1,0,"proposal",2,0,6,0,{}, {},0,0,0,1,2)
ForeignSelection == E(99,4,0,0,"select",2,3,0,0,{4,5},{},0,0,0,1,2)

Work == CASE Scenario \in {"handoff","content","content-invalid","late-fork"} ->
 Seed \o <<Handoff>> \o NewWork(11,6) \o (IF Scenario = "late-fork" THEN <<Fork>> ELSE <<>>)
 [] Scenario = "fork-recovery" -> Seed \o <<Fork,Handoff>> \o NewWork(11,6)
 [] Scenario = "pending-repair" -> Seed \o <<Handoff,Restore(18,3,11,11,11)>> \o NewWork(18,0)
 [] Scenario \in {"invalid-initial","missing-initial"} ->
 <<P(3,1,0,2,0,{}),Restore(11,2,2,2,2)>> \o NewWork(11,0)
 [] Scenario = "repeated-repair" -> Seed \o <<Handoff,
 Epoch(18,3,11,11,99,0,1),Epoch(19,4,18,18,3,0,1),
 Restore(26,5,19,19,18)>> \o NewWork(26,6)
 [] Scenario = "unseeded-fence" -> Seed \o <<Epoch(11,2,2,2,0,0,1)>> \o NewWork(11,0)
 [] Scenario = "null-after-boundary" -> Seed \o <<Handoff,Epoch(18,3,11,11,0,0,1)>> \o NewWork(18,0)
 [] Scenario \in {"source-exclusion","missing-source"} -> Seed \o SourceWork
 [] Scenario = "wrong-source" -> Seed \o <<ForeignSource,P(21,3,1,2,6,{20}),
 P(22,1,1,2,6,{21}),R(23,4,2,2,22),R(24,5,0,2,22),
 S(25,4,1,2,22,6,{23,24})>>
 [] Scenario = "source-cycle" -> Seed \o <<P(20,2,1,2,6,{21}),P(21,3,1,2,6,{20})>>
       \o SubSeq(SourceWork,3,6)
 [] Scenario = "historical-author" -> Seed \o HistoricalWork
 [] Scenario = "review-fork" -> Seed \o <<R(17,2,0,2,3),Handoff>> \o NewWork(11,6)
 [] Scenario = "authority-fork" -> Seed \o <<P(17,4,0,2,0,{}),Handoff>> \o NewWork(11,6)
 [] Scenario = "wrong-predecessor" -> Seed \o <<Handoff>> \o NewWork(11,0)
 [] Scenario = "wrong-goal" -> Seed \o <<Epoch(11,2,2,2,99,0,1),ForeignSelection>>
 [] Scenario = "wrong-evidence" -> Seed \o <<P(22,1,1,2,6,{}),S(25,4,1,2,22,6,{4,5})>>
 [] Scenario = "rules" -> Seed \o <<RulesChanged,P(22,1,1,2,0,{}),R(23,2,1,2,22),
 S(25,5,0,2,22,0,{23})>>
 [] Scenario = "cutoff" -> Seed \o <<Removal,Handoff>> \o NewWork(11,6)
 [] Scenario = "admin-fork" -> Seed \o <<Handoff,AdminFork>> \o NewWork(11,6)
 [] OTHER -> Seed

Transcript == <<G,InitialEpoch>> \o Work
IDs == {Transcript[i].id : i \in 1..Len(Transcript)}
ByID == [id \in IDs |-> CHOOSE e \in {Transcript[i] : i \in 1..Len(Transcript)} : e.id = id]
GovKinds == {"rules","epoch","remove"}
IsGov(id) == ByID[id].kind \in GovKinds
RECURSIVE Ancestors(_, _)
Ancestors(H,id) == IF id = 0 THEN {} ELSE IF id \notin H THEN {id}
 ELSE {id} \cup Ancestors(H,ByID[id].prev)
SameSlot(H,id) == {x \in H : ByID[x].author = ByID[id].author /\ ByID[x].slot = ByID[id].slot}
Usable(H,id) == id \in H /\ Cardinality(SameSlot(H,id)) = 1
Governance(H) == {id \in H : IsGov(id) /\ ByID[id].author = 0 /\
 Ancestors(H,id) \subseteq H /\ \A a \in Ancestors(H,id) : Usable(H,a) /\ IsGov(a)}
Epochs(H) == {id \in Governance(H) : ByID[id].kind = "epoch"}
EpochAncestors(H,id) == Ancestors(H,id) \cap Epochs(H)
PrecedingEpoch(H,id) == LET es == (Ancestors(H,ByID[id].prev) \cap Epochs(H))
 IN IF es = {} THEN 0 ELSE CHOOSE e \in es : \A x \in es : ByID[e].slot >= ByID[x].slot
StructuralEpoch(H,e) == e \in Epochs(H) /\ ByID[e].expected = PrecedingEpoch(H,e)
RECURSIVE Boundary(_)
Boundary(e) == IF e = 0 THEN 0 ELSE
 IF ByID[e].restore # 0 THEN Boundary(ByID[ByID[e].restore].expected)
 ELSE ByID[e].checkpoint
Inherited(e) == Boundary(ByID[e].expected)
BoundaryShape(H,e) ==
 /\ StructuralEpoch(H,e)
 /\ IF ByID[e].restore # 0 THEN
       /\ ByID[e].checkpoint = 0
       /\ ByID[e].restore \in EpochAncestors(H,ByID[e].expected)
    ELSE ByID[e].checkpoint # 0 \/ ~CheckNullBoundary \/ Inherited(e) = 0
CurrentEpoch(H) == LET es == {e \in Epochs(H) : StructuralEpoch(H,e)}
 IN IF es = {} THEN 0 ELSE CHOOSE e \in es : \A x \in es : ByID[e].slot >= ByID[x].slot
Allowed(H,id) == ~CheckCutoff \/ ~\E r \in Governance(H) :
 ByID[r].kind = "remove" /\ ByID[r].subject = ByID[id].author
KnownPolicy(D,e) == ByID[e].rules \in D
Rule(H,e) == IF ~CheckPinnedRules /\ 90 \in Governance(H) THEN 90 ELSE ByID[e].rules
Integrator(rule) == IF rule = 90 THEN 5 ELSE 4
Threshold(rule) == IF rule = 90 THEN 1 ELSE 2

Dependencies(id) == IF IsGov(id) THEN {} ELSE
 ({ByID[id].round,ByID[id].parent,ByID[id].prior,ByID[id].subject} \ {0})
 \cup ByID[id].sources \cup ByID[id].evidence
RECURSIVE Closure(_, _)
Closure(H,roots) == LET more == roots \cup UNION {Dependencies(id) : id \in roots \cap H}
 IN IF more = roots THEN roots ELSE Closure(H,more)
RECURSIVE Sources(_, _)
Sources(H,p) == LET more == p \cup UNION {ByID[id].sources : id \in p \cap H}
 IN IF more = p THEN p ELSE Sources(H,more)
Authors(H,p) == {ByID[s].author : s \in Sources(H,{p}) \cap H}
Compatible(H,pins) == \A a,b \in pins \cap H :
 (ByID[a].author = ByID[b].author /\ ByID[a].slot = ByID[b].slot) => a = b
Proof(H,d) == Closure(H,({ByID[d].round,ByID[d].subject,ByID[d].prior} \ {0}) \cup ByID[d].evidence)
Authorized(H,id,pins) == id \in H /\ ByID[id].goal = 1 /\ Allowed(H,id)
 /\ (Usable(H,id) \/ id \in pins)

RECURSIVE EpochReady(_,_,_), Valid(_,_,_,_), Decision(_,_,_,_)
EpochReady(H,D,e) ==
 /\ e \in IDs /\ BoundaryShape(H,e) /\ KnownPolicy(D,e)
 /\ (Boundary(e) = 0 \/
       (Boundary(e) \in H /\ ByID[Boundary(e)].kind = "select" /\
        ByID[Boundary(e)].round \in EpochAncestors(H,ByID[e].expected) /\
        (~CheckCheckpoint \/ Decision(H,D,Boundary(e),TRUE))))

ProposalValid(H,D,p,pins) ==
 /\ Authorized(H,p,pins) /\ EpochReady(H,D,ByID[p].round)
 /\ (ByID[p].parent = 0 \/ (ByID[p].parent \in H /\ ByID[ByID[p].parent].kind = "select"))
 /\ (~CheckSources \/ (Sources(H,{p}) \subseteq H /\
       \A s \in Sources(H,{p}) : ByID[s].kind = "proposal" /\ ByID[s].goal = 1 /\ Allowed(H,s)
       /\ s \notin Sources(H,ByID[s].sources)))
ReviewValid(H,D,r,pins) ==
 /\ Authorized(H,r,pins) /\ ByID[r].subject \in H
 /\ ByID[ByID[r].subject].kind = "proposal"
 /\ ByID[r].round = ByID[ByID[r].subject].round
 /\ ProposalValid(H,D,ByID[r].subject,pins)
 /\ (~CheckExcludeAuthors \/ ByID[r].author \notin Authors(H,ByID[r].subject))
Reviewers(H,D,d,pins) == {ByID[r].author : r \in {x \in ByID[d].evidence \cap H :
 ByID[x].kind = "review" /\ (~CheckExactEvidence \/ ByID[x].subject = ByID[d].subject) /\
 ReviewValid(H,D,x,pins)}}
Successors(H,d) == {x \in H : ByID[x].kind = "select" /\
 ByID[x].round = ByID[d].round /\ ByID[x].prior = ByID[d].prior /\
 ByID[x].author = Integrator(Rule(H,ByID[d].round)) /\ Allowed(H,x)}
SelectionSlot(H,d,checkpointMode) == Usable(H,d) \/
 (checkpointMode /\ \A x \in SameSlot(H,d) : ByID[x].kind = "select" /\
   ByID[x].round = ByID[d].round /\ ByID[x].prior = ByID[d].prior)
Decision(H,D,d,checkpointMode) ==
 IF d \notin H THEN FALSE ELSE LET e == ByID[d] pins == Proof(H,d)
 IN /\ e.kind = "select" /\ e.goal = 1 /\ Allowed(H,d)
    /\ e.round \in IDs /\ EpochReady(H,D,e.round)
    /\ e.author = Integrator(Rule(H,e.round))
    /\ SelectionSlot(H,d,checkpointMode)
    /\ (checkpointMode \/ Cardinality(Successors(H,d)) = 1)
    /\ pins \subseteq H /\ Compatible(H,pins)
    /\ e.subject \in H /\ ByID[e.subject].kind = "proposal"
    /\ ByID[e.subject].round = e.round /\ ProposalValid(H,D,e.subject,pins)
    /\ ByID[e.subject].parent = IF e.prior = 0 THEN Boundary(e.round) ELSE e.prior
    /\ (e.prior = 0 \/ (ByID[e.prior].round = e.round /\ Decision(H,D,e.prior,checkpointMode)))
    /\ Cardinality(Reviewers(H,D,d,pins)) >= Threshold(Rule(H,e.round))
Valid(H,D,id,pins) == IF id \notin H THEN FALSE ELSE
 CASE ByID[id].kind = "proposal" -> ProposalValid(H,D,id,pins)
 [] ByID[id].kind = "review" -> ReviewValid(H,D,id,pins)
 [] ByID[id].kind = "select" -> Decision(H,D,id,FALSE)
 [] OTHER -> id \in Governance(H)
Selected(H,D) == {d \in H : ByID[d].kind = "select" /\ Decision(H,D,d,FALSE)}
GlobalPins(H,D) == IF ScopeIsolation THEN {} ELSE UNION {Proof(H,d) : d \in Selected(H,D)}
Ordinary(H,D) == {id \in H : ByID[id].kind \in {"proposal","review"} /\ Valid(H,D,id,GlobalPins(H,D))}
ActiveSelections(H,D,e) == {d \in Selected(H,D) : ~CheckFence \/ ByID[d].round = e}
WorkspaceHead(H,D,e) == LET ds == ActiveSelections(H,D,e)
 IN IF ds = {} THEN IF EpochReady(H,D,e) THEN Boundary(e) ELSE 0
 ELSE CHOOSE d \in ds : \A x \in ds : ByID[d].slot >= ByID[x].slot
Authority(H,D) == LET e == CurrentEpoch(H)
 IN [epoch |-> e, ready |-> e # 0 /\ EpochReady(H,D,e),
     head |-> IF e = 0 THEN 0 ELSE WorkspaceHead(H,D,e), ordinary |-> Ordinary(H,D),
     selected |-> Selected(H,D), governance |-> Governance(H)]
Projection(H,D,O) == LET a == Authority(H,D)
 IN IF RequireContent /\ a.head # 0 /\ ByID[a.head].subject \notin O
 THEN [a EXCEPT !.head = 0] ELSE a

ContentState(V,O) == IF V.head = 0 THEN "NoRevision" ELSE
 IF ByID[V.head].subject \notin O THEN "Missing" ELSE
 IF Scenario \in {"content","content-invalid"} /\ ByID[V.head].subject = 3 THEN "Invalid"
 ELSE "Ready"
VARIABLES held, definitions, objects, view, contentState, witnessReached, faultPresent
vars == <<held,definitions,objects,view,contentState,witnessReached,faultPresent>>
Witness(H,D,V,O) == CASE Scenario \in {"seed","missing-policy"} -> V.head = 6
 [] Scenario \in {"handoff","late-fork","fork-recovery","review-fork"} ->
 V.epoch = 11 /\ V.head = 15 /\ (Scenario \notin {"late-fork","fork-recovery"} \/ 16 \in H)
 [] Scenario = "content" -> V.head = 15 /\ 12 \in O /\ 3 \notin O
 [] Scenario = "content-invalid" -> V.head = 6 /\ ContentState(V,O) = "Invalid"
 [] Scenario = "pending-repair" -> V.epoch = 18 /\ V.head = 15 /\ 6 \in H
 [] Scenario \in {"invalid-initial","missing-initial"} -> V.epoch = 11 /\ V.head = 15
 [] Scenario = "repeated-repair" -> V.epoch = 26 /\ V.head = 15 /\ {18,19} \subseteq H
 [] Scenario = "unseeded-fence" -> V.epoch = 11 /\ V.head = 15 /\ 6 \in H
 [] Scenario = "null-after-boundary" -> V.epoch = 18 /\ ~V.ready /\ V.head = 0
 [] Scenario \in {"source-exclusion","wrong-source","source-cycle","missing-source","wrong-evidence"} ->
 25 \in H /\ V.head = 6 /\ 25 \notin V.selected
 [] Scenario = "historical-author" -> V.head = 25
 [] Scenario = "authority-fork" -> 17 \in H /\ V.epoch = 11 /\ ~V.ready
 [] Scenario = "wrong-predecessor" -> 15 \in H /\ V.head = 6 /\ 15 \notin V.selected
 [] Scenario = "wrong-goal" -> 99 \in H /\ V.epoch = 11 /\ ~V.ready
 [] Scenario = "rules" -> 90 \in H /\ 25 \in H /\ V.head = 6 /\ 25 \notin V.selected
 [] Scenario = "cutoff" -> 91 \in H /\ V.epoch = 11 /\ ~V.ready
 [] Scenario = "admin-fork" -> 92 \in H /\ V.epoch = 0 /\ V.head = 0
 [] OTHER -> FALSE
Fault(H) == CASE Scenario = "missing-source" -> 21 \in H /\ 20 \notin H
 [] Scenario = "pending-repair" -> 11 \in H /\ 6 \notin H
 [] Scenario \in {"invalid-initial","missing-initial"} -> 11 \notin H
 [] Scenario \in {"content","content-invalid","cutoff"} -> 6 \in H
 [] Scenario = "wrong-goal" -> {11,99} \subseteq H
 [] Scenario = "unseeded-fence" -> {11,6} \subseteq H
 [] Scenario = "null-after-boundary" -> 18 \in H
 [] Scenario \in {"source-exclusion","wrong-source","source-cycle","wrong-evidence"} -> 25 \in H
 [] Scenario = "review-fork" -> 17 \in H
 [] Scenario = "rules" -> {90,25} \subseteq H
 [] OTHER -> (IDs \ {1,2}) \subseteq H
Init == /\ held = {1,2} /\ definitions = IF Scenario = "missing-policy" THEN {} ELSE {1,90}
 /\ objects = {} /\ view = Projection(held,definitions,objects)
 /\ contentState = ContentState(view,objects) /\ witnessReached = FALSE /\ faultPresent = Fault(held)
Deliver(id) == /\ id \in IDs \ held /\ held' = held \cup {id}
 /\ UNCHANGED <<definitions,objects>> /\ view' = Projection(held',definitions,objects)
 /\ contentState' = ContentState(view',objects)
 /\ witnessReached' = Witness(held',definitions,view',objects) /\ faultPresent' = Fault(held')
ReceiveContent(id) == /\ Scenario \in {"content","content-invalid"} /\ id \in {3,12} \ objects
 /\ objects' = objects \cup {id} /\ UNCHANGED <<held,definitions>>
 /\ view' = Projection(held,definitions,objects')
 /\ contentState' = ContentState(view',objects')
 /\ witnessReached' = Witness(held,definitions,view',objects') /\ faultPresent' = Fault(held)
ReceivePolicy == /\ definitions = {} /\ definitions' = {1,90} /\ UNCHANGED <<held,objects>>
 /\ view' = Projection(held,definitions',objects) /\ contentState' = ContentState(view',objects)
 /\ witnessReached' = Witness(held,definitions',view',objects) /\ faultPresent' = Fault(held)
Next == (\E id \in IDs : Deliver(id)) \/ (\E id \in {3,12} : ReceiveContent(id))
 \/ ReceivePolicy \/ UNCHANGED vars
Spec == Init /\ [][Next]_vars

TypeOK == /\ held \subseteq IDs /\ definitions \subseteq {1,90} /\ objects \subseteq {3,12}
 /\ view.head \in held \cup {0} /\ view.epoch \in held \cup {0} /\ witnessReached \in BOOLEAN
 /\ contentState \in {"NoRevision","Missing","Invalid","Ready"}
ReplayMatchesHeld == view = Projection(held,definitions,objects)
AuthorityIndependentOfContent == view = Authority(held,definitions)
NamedPinnedAuthority == \A d \in view.selected : ByID[d].author = Integrator(ByID[ByID[d].round].rules)
ExactCandidateEvidence == \A d \in view.selected :
 \A r \in ByID[d].evidence : ByID[r].kind = "review" /\ ByID[r].subject = ByID[d].subject
SourceClosureRequired == \A d \in view.selected : Sources(held,{ByID[d].subject}) \subseteq held /\
 \A s \in Sources(held,{ByID[d].subject}) : ByID[s].kind = "proposal" /\ ByID[s].goal = 1 /\
 s \notin Sources(held,ByID[s].sources)
SourceAuthorsExcluded == \A d \in view.selected :
 \A r \in ByID[d].evidence : ByID[r].author \notin Authors(held,ByID[d].subject)
CutoffFencesEvidence == \A d \in view.selected : \A r \in ByID[d].evidence :
 ~\E m \in view.governance : ByID[m].kind = "remove" /\ ByID[m].subject = ByID[r].author
EpochFence == view.head = 0 \/ view.head = Boundary(view.epoch) \/ ByID[view.head].round = view.epoch
CheckpointHasAuthority == (view.ready /\ Boundary(view.epoch) # 0) =>
 Boundary(view.epoch) \in held /\ ByID[Boundary(view.epoch)].kind = "select" /\
 ByID[Boundary(view.epoch)].goal = 1 /\ Decision(held,definitions,Boundary(view.epoch),TRUE)
OrdinaryNullCannotErase == (view.ready /\ ByID[view.epoch].checkpoint = 0 /\ ByID[view.epoch].restore = 0)
 => Inherited(view.epoch) = 0
NoGlobalForkAuthority == \A id \in view.ordinary : Usable(held,id)
GovernanceIgnoresWork == view.governance = Governance(held)
NeverWitness == ~witnessReached
=============================================================================
