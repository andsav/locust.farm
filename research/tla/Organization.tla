---------------------------- MODULE Organization ----------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(***************************************************************************
Current organization protocol subset at c88e3bc. Immutable authenticated events
and definition hashes are abstract identifiers. Each finite scenario chooses a
signed transcript; TLC explores every delivery order and duplicate/stutter from
an already verified founding prefix. This is not arbitrary message generation,
a cryptographic proof, or a proof that the Rust code refines this specification.

The model includes a narrow administrator log, exact admission cutoffs, pinned
rule revisions, taskless contributions, distinct positive review identities,
scope-local exact proof closure and named selection authorities. Ordinary work
never gains authority merely because another scope selected its author branch.
Document bodies, general selectors, closure/reopen, encrypted epochs, malformed
wire decoding and transport are excluded. Rust regressions cover those seams.
***************************************************************************)

CONSTANTS Scenario, EnablePins, ScopeIsolation, CheckCutoff, CheckDistinct,
          CheckScope, CheckAuthority, CheckRulePin, RequireDefinition

E(id, author, seq, prev, anchor, kind, scope, round, subject, evidence,
  prior, member, admission, cutoff, rules) ==
 [id |-> id, author |-> author, seq |-> seq, prev |-> prev, anchor |-> anchor,
  kind |-> kind, scope |-> scope, round |-> round, subject |-> subject,
  evidence |-> evidence, prior |-> prior, member |-> member,
  admission |-> admission, cutoff |-> cutoff, rules |-> rules]

Founding == <<
 E(1,0,0,0,0,"genesis",0,0,0,{},0,0,0,0,0),
 E(2,0,1,1,1,"admit",0,0,0,{},0,0,0,0,0),
 E(3,0,2,2,2,"admit",0,0,0,{},0,1,0,0,0),
 E(4,0,3,3,3,"admit",0,0,0,{},0,2,0,0,0),
 E(5,0,4,4,4,"admit",0,0,0,{},0,3,0,0,0),
 E(6,0,5,5,5,"admit",0,0,0,{},0,4,0,0,0),
 E(7,0,6,6,6,"rules",0,7,0,{},0,0,0,0,7)>>
Task == E(8,1,0,0,7,"task",8,8,0,{},0,0,0,0,7)
Candidate == E(9,1,1,8,7,"contribution",8,8,0,{},0,0,0,0,7)
ReviewA == E(10,2,0,0,7,"review",8,8,9,{},0,0,0,0,7)
ReviewB == E(11,3,0,0,7,"review",8,8,9,{},0,0,0,0,7)
Selection == E(12,4,0,0,7,"select",8,8,9,{10,11},0,0,0,0,7)
Independent == E(13,1,2,9,7,"contribution",0,7,0,{},0,0,0,0,7)
Base == <<Task, Candidate, ReviewA, ReviewB, Selection, Independent>>
ForkTask == E(14,1,0,0,7,"task",14,14,0,{},0,0,0,0,7)
ForkReview == E(14,2,0,0,7,"review",8,8,9,{},0,0,0,0,7)
Remove(cutoff) == E(15,0,7,7,7,"remove",0,0,0,{},0,2,4,cutoff,0)
Readmit == E(16,0,8,15,15,"admit",0,0,0,{},0,2,0,0,0)
OtherBranch == <<ForkTask,
 E(15,1,1,14,7,"contribution",14,14,0,{},0,0,0,0,7),
 E(16,2,0,0,7,"review",14,14,15,{},0,0,0,0,7),
 E(17,3,0,0,7,"review",14,14,15,{},0,0,0,0,7),
 E(18,4,1,12,7,"select",14,14,15,{16,17},0,0,0,0,7)>>

Work == CASE Scenario = "taskless" -> <<
 E(8,1,0,0,7,"contribution",0,7,0,{},0,0,0,0,7),
 E(9,2,0,0,7,"contribution",0,7,0,{},0,0,0,0,7)>>
 [] Scenario = "fork" -> Base \o <<ForkTask>>
 [] Scenario = "review-fork" -> Base \o <<ForkReview>>
 [] Scenario = "scope-conflict" -> Base \o <<
 E(14,4,1,12,7,"select",8,8,9,{10,11},0,0,0,0,7)>>
 [] Scenario = "authority-fork" -> Base \o <<
 E(14,4,0,0,7,"select",8,8,9,{10,11},0,0,0,0,7)>>
 [] Scenario = "duplicate-review" -> <<Task,Candidate,ReviewA,
 E(14,2,1,10,7,"review",8,8,9,{},0,0,0,0,7),
 E(12,4,0,0,7,"select",8,8,9,{10,14},0,0,0,0,7)>>
 [] Scenario = "rule-change" -> <<Task,Candidate,ReviewA,
 E(14,0,7,7,7,"rules",0,14,0,{},0,0,0,0,14),
 E(12,4,0,0,14,"select",8,8,9,{10},0,0,0,0,7)>>
 [] Scenario = "cutoff" -> Base \o <<ForkReview,Remove(10),Readmit>>
 [] Scenario = "remove-empty" -> Base \o <<Remove(0),Readmit>>
 [] Scenario = "governance-fork" -> Base \o <<
 E(14,0,3,3,3,"admit",0,0,0,{},0,4,0,0,0)>>
 [] Scenario = "forged-selection" -> <<Task,Candidate,ReviewA,ReviewB,
 E(12,1,2,9,7,"select",8,8,9,{10,11},0,0,0,0,7)>>
 [] Scenario = "wrong-scope" -> <<Task,Candidate,ReviewA,ReviewB,
 E(14,1,2,9,7,"task",14,14,0,{},0,0,0,0,7),
 E(12,4,0,0,7,"select",14,14,9,{10,11},0,0,0,0,7)>>
 [] Scenario = "two-scopes" -> SubSeq(Base,1,5) \o OtherBranch
 [] Scenario = "incompatible" -> SubSeq(Base,1,5) \o OtherBranch \o <<
 E(19,4,2,18,7,"select",8,8,9,{10,11,16},12,0,0,0,7)>>
 [] OTHER -> Base

Transcript == Founding \o Work
IDs == {Transcript[i].id : i \in 1..Len(Transcript)}
ByID == [id \in IDs |-> CHOOSE e \in {Transcript[i] : i \in 1..Len(Transcript)} : e.id = id]
GovKinds == {"genesis","admit","remove","rules"}
IsGov(id) == ByID[id].kind \in GovKinds

RECURSIVE Ancestors(_, _)
Ancestors(H,id) ==
 IF id = 0 THEN {}
 ELSE IF id \notin H THEN {id}
 ELSE {id} \cup Ancestors(H,ByID[id].prev)

AuthorChain(H,id) ==
 /\ Ancestors(H,id) \subseteq H
 /\ \A a \in Ancestors(H,id) :
     IF ByID[a].prev = 0 THEN ByID[a].seq = 0
     ELSE /\ ByID[ByID[a].prev].author = ByID[a].author
          /\ ByID[ByID[a].prev].seq + 1 = ByID[a].seq
UniquePositions(H,id) ==
 \A a \in Ancestors(H,id) \cap H :
    Cardinality({b \in H : ByID[b].author = ByID[a].author /\ ByID[b].seq = ByID[a].seq}) = 1
Usable(H,id) == AuthorChain(H,id) /\ UniquePositions(H,id)
Governance(H) == {id \in H : IsGov(id) /\ ByID[id].author = 0 /\ Usable(H,id)
 /\ \A a \in Ancestors(H,id) : ByID[a].anchor = ByID[a].prev}

Admission(H,p,anchor) == {a \in Governance(H) :
 /\ ByID[a].kind = "admit" /\ ByID[a].member = p
 /\ ByID[a].seq <= ByID[anchor].seq
 /\ ~\E r \in Governance(H) : ByID[r].kind = "remove" /\
       ByID[r].admission = a /\ ByID[r].seq <= ByID[anchor].seq}
MemberAt(H,p,anchor) == anchor \in Governance(H) /\ Admission(H,p,anchor) # {}
CutoffAllows(H,id) ==
 \A r \in Governance(H) :
  (ByID[r].kind = "remove" /\ ByID[r].admission \in Admission(H,ByID[id].author,ByID[id].anchor))
   => ByID[r].cutoff # 0 /\ id \in Ancestors(H,ByID[r].cutoff)
RetainedByCutoff(H,id) ==
 \E r \in Governance(H) : ByID[r].kind = "remove" /\
    ByID[r].admission \in Admission(H,ByID[id].author,ByID[id].anchor) /\
    ByID[r].cutoff # 0 /\ id \in Ancestors(H,ByID[r].cutoff)
AnchorsMonotonic(H,id) ==
 \A a,b \in Ancestors(H,id) :
  (ByID[a].seq < ByID[b].seq /\ ByID[a].anchor # 0 /\ ByID[b].anchor # 0)
   => ByID[a].anchor \in Governance(H) /\ ByID[b].anchor \in Governance(H) /\
      ByID[ByID[a].anchor].seq <= ByID[ByID[b].anchor].seq
Eligible(H,id) == /\ id \in H /\ AuthorChain(H,id)
 /\ MemberAt(H,ByID[id].author,ByID[id].anchor)
 /\ AnchorsMonotonic(H,id)
 /\ (~CheckCutoff \/ CutoffAllows(H,id))
Authorized(H,id,pins) == Eligible(H,id) /\
 (Usable(H,id) \/ RetainedByCutoff(H,id) \/
  (EnablePins /\ id \in pins /\ ByID[id].kind # "select"))

Dependencies(id) ==
 IF IsGov(id) THEN {}
 ELSE ({ByID[id].prev,ByID[id].round,ByID[id].subject,ByID[id].prior} \ {0}) \cup ByID[id].evidence
RECURSIVE Closure(_, _)
Closure(H,roots) ==
 LET more == roots \cup UNION {Dependencies(id) : id \in roots \cap H}
 IN IF more = roots THEN roots ELSE Closure(H,more)
Proof(H,d) == Closure(H,({ByID[d].round,ByID[d].subject,ByID[d].prior} \ {0}) \cup ByID[d].evidence)
Compatible(H,pins) == \A a,b \in pins \cap H :
 (ByID[a].author = ByID[b].author /\ ByID[a].seq = ByID[b].seq) => a = b
Threshold(rule) == IF rule = 14 THEN 1 ELSE 2
KnownRule(D,rule) == ~RequireDefinition \/ rule \in D
RuleFor(H,c) == IF CheckRulePin THEN ByID[ByID[c].round].rules
 ELSE IF 14 \in Governance(H) /\ ByID[14].kind = "rules" THEN 14 ELSE 7

RECURSIVE Valid(_,_,_,_), Decision(_,_,_)
Reviewers(H,D,c,pins,roots) == {ByID[r].author : r \in {v \in roots \cap H :
 ByID[v].kind = "review" /\ ByID[v].subject = c /\ Valid(H,D,v,pins)}}
ReviewEvents(H,D,c,pins,roots) == {r \in roots \cap H :
 ByID[r].kind = "review" /\ ByID[r].subject = c /\ Valid(H,D,r,pins)}
Approved(H,D,c,pins,roots) ==
 (IF CheckDistinct THEN Cardinality(Reviewers(H,D,c,pins,roots))
  ELSE Cardinality(ReviewEvents(H,D,c,pins,roots))) >= Threshold(RuleFor(H,c))

Conflicts(H,d) == {other \in H : ByID[other].kind = "select" /\
 ByID[other].author = ByID[d].author /\ ByID[other].scope = ByID[d].scope /\
 ByID[other].round = ByID[d].round /\ ByID[other].prior = ByID[d].prior /\ Eligible(H,other)}
Decision(H,D,d) ==
 LET e == ByID[d]
     pins == Proof(H,d)
 IN /\ (~CheckAuthority \/ e.author = 4)
    /\ Authorized(H,d,{}) /\ Cardinality(Conflicts(H,d)) = 1
    /\ pins \subseteq H /\ d \notin pins /\ Compatible(H,pins)
    /\ \A r \in pins : (ByID[r].author = e.author => ByID[r].seq < e.seq)
    /\ \A r \in pins : ByID[r].anchor = 0 \/
           (ByID[r].anchor \in Governance(H) /\ ByID[ByID[r].anchor].seq <= ByID[e.anchor].seq)
    /\ (e.prior = 0 \/ (ByID[e.prior].scope = e.scope /\
         ByID[e.prior].round = e.round /\ Valid(H,D,e.prior,{})))
    /\ Valid(H,D,e.round,pins) /\ Valid(H,D,e.subject,pins)
    /\ ByID[e.subject].kind = "contribution"
    /\ (~CheckScope \/ (ByID[e.subject].scope = e.scope /\ ByID[e.subject].round = e.round))
    /\ \A r \in e.evidence : Valid(H,D,r,pins)
    /\ Approved(H,D,e.subject,pins,e.evidence)

Valid(H,D,id,pins) ==
 IF id \notin H THEN FALSE
 ELSE LET e == ByID[id]
 IN IF IsGov(id) THEN id \in Governance(H) /\ (e.kind # "rules" \/ KnownRule(D,e.rules))
 ELSE IF e.kind = "select" THEN Decision(H,D,id)
 ELSE /\ Authorized(H,id,pins)
      /\ CASE e.kind = "task" -> Valid(H,D,e.rules,pins)
         [] e.kind = "contribution" -> Valid(H,D,e.round,pins)
         [] e.kind = "review" ->
              /\ e.author \in {2,3} /\ e.author # ByID[e.subject].author
              /\ Valid(H,D,e.round,pins) /\ Valid(H,D,e.subject,pins)
              /\ ByID[e.subject].kind = "contribution"
              /\ e.scope = ByID[e.subject].scope /\ e.round = ByID[e.subject].round
         [] OTHER -> FALSE

Selected(H,D) == {d \in H : ByID[d].kind = "select" /\ Decision(H,D,d)}
GlobalPins(H,D) == IF ScopeIsolation THEN {} ELSE UNION {Proof(H,d) : d \in Selected(H,D)}
Projection(H,D) == [ordinary |-> {id \in H : ~IsGov(id) /\ ByID[id].kind # "select" /\ Valid(H,D,id,GlobalPins(H,D))},
 selected |-> Selected(H,D), governance |-> Governance(H)]

VARIABLES held, definitions, view, witnessReached, faultPresent
vars == <<held, definitions, view, witnessReached, faultPresent>>
Witness(H,D,V) == CASE Scenario = "taskless" -> {8,9} \subseteq V.ordinary /\ V.selected = {}
 [] Scenario \in {"fork","review-fork"} -> {12,14} \subseteq H /\ 12 \in V.selected
 [] Scenario = "two-scopes" -> {12,18} \subseteq V.selected /\ V.ordinary = {}
 [] Scenario = "incompatible" -> {12,18,19} \subseteq H /\ {12,18} \subseteq V.selected /\ 19 \notin V.selected
 [] Scenario = "missing-definition" -> 7 \in D /\ 12 \in V.selected
 [] Scenario = "cutoff" -> {14,15,16} \subseteq H /\ 10 \in V.ordinary /\ 14 \notin V.ordinary /\ 12 \in V.selected
 [] Scenario = "remove-empty" -> {12,15,16} \subseteq H /\ 10 \notin V.ordinary /\ 12 \notin V.selected
 [] Scenario \in {"scope-conflict","authority-fork"} -> {12,14} \subseteq H /\ V.selected = {}
 [] OTHER -> 12 \in V.selected
Fault(H,D) == CASE Scenario = "missing-definition" -> 7 \notin D
 [] Scenario = "remove-empty" -> 15 \in H
 [] Scenario \in {"fork","review-fork","rule-change","two-scopes"} -> 14 \in H
 [] OTHER -> 12 \in H
Init == /\ held = 1..7 /\ definitions = IF Scenario = "missing-definition" THEN {} ELSE {7,14}
        /\ view = Projection(held,definitions) /\ witnessReached = FALSE /\ faultPresent = Fault(held,definitions)
Deliver(id) == /\ id \in IDs \ held /\ held' = held \cup {id}
 /\ UNCHANGED definitions /\ view' = Projection(held',definitions)
 /\ witnessReached' = Witness(held',definitions,view') /\ faultPresent' = Fault(held',definitions)
ReceiveDefinition == /\ 7 \notin definitions /\ definitions' = definitions \cup {7}
 /\ UNCHANGED held /\ view' = Projection(held,definitions')
 /\ witnessReached' = Witness(held,definitions',view') /\ faultPresent' = Fault(held,definitions')
Next == (\E id \in IDs : Deliver(id)) \/ ReceiveDefinition \/ UNCHANGED vars
Spec == Init /\ [][Next]_vars

TypeOK == /\ held \subseteq IDs /\ definitions \subseteq {7,14}
 /\ view.ordinary \subseteq held /\ view.selected \subseteq held /\ witnessReached \in BOOLEAN
GovernanceAuthority == \A id \in view.governance : ByID[id].author = 0
NamedAuthority == \A d \in view.selected : ByID[d].author = 4
ExactScope == \A d \in view.selected : ByID[d].scope = ByID[ByID[d].subject].scope /\
 ByID[d].round = ByID[ByID[d].subject].round
DistinctPinnedQuorum == \A d \in view.selected :
 Cardinality(Reviewers(held,definitions,ByID[d].subject,Proof(held,d),ByID[d].evidence)) >=
 Threshold(ByID[ByID[ByID[d].subject].round].rules)
NoGlobalForkAuthority == \A id \in view.ordinary : Usable(held,id) \/ RetainedByCutoff(held,id)
CutoffFencesEvidence == \A d \in view.selected :
 \A r \in ByID[d].evidence : CutoffAllows(held,r)
MissingDefinitionWaits == 7 \notin definitions => view.ordinary = {} /\ view.selected = {}
ScopedProofSurvivesFork ==
 (Scenario \in {"fork","review-fork"} /\ {8,9,10,11,12,14} \subseteq held /\ 7 \in definitions)
 => 12 \in view.selected
DoubleSuccessorHalts == \A d \in held :
 (ByID[d].kind = "select" /\ Cardinality(Conflicts(held,d)) > 1) => d \notin view.selected
ReplayMatchesHeld == view = Projection(held,definitions)
NeverWitness == ~witnessReached
=============================================================================
