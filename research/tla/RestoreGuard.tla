---------------------------- MODULE RestoreGuard ----------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(* Proposed G1/G2, one local daemon L and peers P,Q; NOT record validity.
   Signatures are fresh IDs with per-key positions. Atomic writes put the mark
   on durable storage before any peer can receive the record. Sync is a complete
   record stage: agent records precede governance and are dropped if the agent's
   admission is absent at its start. A second exchange can therefore add data.
   File identities are abstract facts, not a filesystem qualification.
   One copy/restore, one admission and one removal, bounded signatures; no
   clock. A second removal is possible only after Continue, which only the
   override and leave traces enable. *)
CONSTANTS Host, InitialPeers, InitialAgent, MaxRecords, Scenario, Mutation,
          RestoreKind, AllowContinue
L == "local"
P == "peer1"
Q == "peer2"
Nodes == {L,P,Q}
Peers == {P,Q}
Keys == {"gov","agent"}
Owner(k) == IF k = "gov" THEN Host ELSE L
ASSUME /\ Host \in {L,P} /\ InitialPeers \subseteq Peers
       /\ MaxRecords >= 3
Root == [key |-> "gov", pos |-> 0, members |-> InitialPeers,
         admitted |-> InitialAgent, kind |-> "root"]
VARIABLES s, phase, forkG, forkA, reusedG, reusedA, agentWhileHeld,
          gaveWhileHeld, ordinaryHeld, govHeld, agentHeld
vars == <<s,phase,forkG,forkA,reusedG,reusedA,agentWhileHeld,gaveWhileHeld,ordinaryHeld,govHeld,agentHeld>>
Ids(h,k,d) == {i \in d : h[i].key = k}
Tip(h,k,d) == IF Ids(h,k,d) = {} THEN 0 ELSE
 CHOOSE i \in Ids(h,k,d) : \A j \in Ids(h,k,d) : i >= j
Members(h,d) == h[Tip(h,"gov",d)].members
Admitted(h,d) == h[Tip(h,"gov",d)].admitted
Position(h,k,d) == LET t == Tip(h,k,d) IN IF t = 0 THEN 0 ELSE h[t].pos + 1
Observe(action) == IF Scenario = "general" THEN "unobserved" ELSE action
Own == IF Host = L THEN Keys ELSE {"agent"}
Behind(t,k) == t.marks[k] # 0 /\ t.marks[k] \notin t.db[L]
GovHeld(t) == Host = L /\ (Behind(t,"gov") \/ t.unheard)
AgentOwnHeld(t) == Behind(t,"agent") \/ t.unheard \/ t.admittedHold
AgentHeld(t) == AgentOwnHeld(t) \/ (Mutation # "agent-gate" /\ GovHeld(t))
Held(t,k) == IF k = "gov" THEN GovHeld(t) ELSE AgentHeld(t)
Sources(t) == Members(t.hist,t.db[L])
Fork(h,d,k) == \E i,j \in Ids(h,k,d) : i # j /\ h[i].pos = h[j].pos
AllHeld(t) == UNION {t.db[n] : n \in Nodes}
NeverShared(t) == ~t.shared /\ Sources(t) = {}

Init ==
 /\ s = [hist |-> <<Root>>, db |-> [n \in Nodes |-> {1}],
         marks |-> [k \in Keys |-> IF k = "gov" /\ Host = L THEN 1 ELSE 0],
         shared |-> InitialPeers # {}, copyDB |-> {},
         copyMarks |-> [k \in Keys |-> 0], copyShared |-> FALSE,
         copied |-> FALSE, restored |-> "none", running |-> TRUE,
         sameFile |-> TRUE, kept |-> TRUE, unheard |-> FALSE,
         admittedHold |-> FALSE, heard |-> {}, pending |-> "none",
         brought |-> FALSE, reachable |-> {}, didAdmit |-> FALSE,
         didRemove |-> FALSE, removedAfterCopy |-> FALSE,
         continued |-> FALSE, last |-> "init", signedKey |-> "none",
         signedHeld |-> FALSE, gaveHeld |-> FALSE, ordinaryBad |-> FALSE,
         left |-> "none", asked |-> FALSE]
 /\ phase = 0 /\ forkG = FALSE /\ forkA = FALSE
 /\ reusedG = FALSE /\ reusedA = FALSE /\ agentWhileHeld = FALSE
 /\ gaveWhileHeld = FALSE /\ ordinaryHeld = FALSE
 /\ govHeld = FALSE /\ agentHeld = FALSE

SignRecord(k,kind,members,admitted) ==
 /\ s.running /\ Len(s.hist) < MaxRecords
 /\ (Owner(k) # L \/ ~Held(s,k))
 /\ (k = "gov" \/ Admitted(s.hist,s.db[L]))
 /\ ~Fork(s.hist,s.db[Owner(k)],k)
 /\ LET id == Len(s.hist)+1
        r == [key |-> k, pos |-> Position(s.hist,k,s.db[Owner(k)]),
              members |-> members, admitted |-> admitted, kind |-> kind]
    IN s' = [s EXCEPT !.hist = Append(@,r),
        !.db[Owner(k)] = @ \cup {id},
        !.marks = IF Owner(k) = L THEN [@ EXCEPT ![k] = id] ELSE @,
        !.shared = @ \/ (Owner(k) = L /\ members # {}),
        !.didAdmit = @ \/ kind = "admit", !.didRemove = @ \/ kind = "remove",
        !.removedAfterCopy = @ \/ (kind = "remove" /\ s.copied),
        !.last = Observe(kind), !.signedKey = k, !.signedHeld = (k = "agent" /\ GovHeld(s))]
Sign(k) == SignRecord(k,"sign",Members(s.hist,s.db[Owner(k)]),
                                    Admitted(s.hist,s.db[Owner(k)]))
Admit(p) == /\ ~s.didAdmit
 /\ IF p = "agent" THEN
       /\ ~Admitted(s.hist,s.db[Host])
       /\ SignRecord("gov","admit",Members(s.hist,s.db[Host]),TRUE)
    ELSE /\ p \in Peers \ Members(s.hist,s.db[Host])
         /\ SignRecord("gov","admit",Members(s.hist,s.db[Host]) \cup {p},
                       Admitted(s.hist,s.db[Host]))
(* E2: the host signs the removal that follows a leave only once an exchange
   with the leaver's computer, opened by this process, has ended. *)
Remove(p) == /\ (~s.didRemove \/ s.continued) /\ p \in Members(s.hist,s.db[Host])
 /\ p # Host /\ (s.left # p \/ s.asked)
 /\ SignRecord("gov","remove",Members(s.hist,s.db[Host]) \ {p},
               Admitted(s.hist,s.db[Host]))
Leave(p) == /\ s.left = "none" /\ p \in Members(s.hist,s.db[Host]) /\ p # Host
            /\ s' = [s EXCEPT !.left = p, !.asked = FALSE, !.last = Observe("leave")]
Copy == /\ s.running /\ ~s.copied
 /\ s' = [s EXCEPT !.copyDB = s.db[L], !.copyMarks = s.marks,
          !.copyShared = s.shared, !.copied = TRUE, !.last = Observe("copy")]
Restore(kind) == /\ s.copied /\ s.restored = "none"
 /\ (kind = "all" \/ s.kept)
 /\ s' = [s EXCEPT !.db[L] = s.copyDB, !.restored = kind,
       !.marks = IF kind = "all" THEN s.copyMarks ELSE @,
       !.shared = IF kind = "all" THEN s.copyShared ELSE @,
       !.sameFile = FALSE, !.kept = (kind # "all" /\ s.kept),
       !.running = FALSE, !.reachable = {}, !.heard = {}, !.pending = "none",
       !.last = Observe("restore")]
RestoreStore == Restore("store")
RestoreAll == Restore("all")
LoseMarks == /\ s.kept /\ s.restored = "none"
 /\ s' = [s EXCEPT !.kept = FALSE, !.running = FALSE, !.last = Observe("lose-marks")]
Restart == /\ s.running /\ s.last # "restart"
 /\ s' = [s EXCEPT !.running = FALSE, !.last = Observe("restart")]
Start == /\ ~s.running
 /\ LET unknown == ~s.kept /\ ~s.sameFile
        marks == [k \in Keys |-> IF k \notin Own THEN 0
           ELSE IF ~s.kept \/ s.marks[k] < Tip(s.hist,k,s.db[L])
                THEN Tip(s.hist,k,s.db[L]) ELSE s.marks[k]]
        u == s.unheard \/ unknown
        ordinary == s.sameFile /\ ~s.unheard /\ ~s.admittedHold /\
                     \A k \in Own : ~Behind(s,k)
    IN s' = [s EXCEPT !.marks = marks, !.unheard = u,
        !.shared = IF s.kept THEN @ ELSE \E i \in s.db[L] : s.hist[i].members # {},
        !.kept = TRUE, !.sameFile = TRUE, !.running = TRUE,
        !.heard = {}, !.pending = "none", !.asked = FALSE, !.last = Observe("start"),
        !.ordinaryBad = ordinary /\ (u \/ \E k \in Own : marks[k] # 0 /\ marks[k] \notin s.db[L])]

(* Reachability is eventually permanent; honest peers send everything they hold
   unless their own current membership says they were removed. Unknown callers
   can return records even if the restored host's copy does not yet list them. *)
Reach(p) == /\ p \notin s.reachable
 /\ s' = [s EXCEPT !.reachable = @ \cup {p}, !.last = Observe("reach")]
Sync(a,b) ==
 /\ a \in Nodes /\ b \in Nodes /\ a # b /\ s.running
 /\ (a = L \/ a \in s.reachable) /\ (b = L \/ b \in s.reachable)
 /\ (b # L \/ s.pending = "none")
 /\ (b = L \/ \E i \in s.db[a] : b \in s.hist[i].members)
 /\ LET offered == IF a # Host /\ a # L /\ a \notin Members(s.hist,s.db[a])
                    THEN {} ELSE s.db[a]
        accepted == {i \in offered : s.hist[i].key = "gov" \/
                       (Admitted(s.hist,s.db[b]) /\ (b = L \/ b = Host \/ b \in Members(s.hist,s.db[a])))}
        added == accepted \ s.db[b]
        joins == b = L /\ ~Admitted(s.hist,s.db[L]) /\ Admitted(s.hist,s.db[L] \cup accepted)
    IN s' = [s EXCEPT !.db[b] = @ \cup accepted,
        !.pending = IF b = L THEN a ELSE @,
        !.brought = IF b = L THEN added # {} ELSE @,
        !.admittedHold = @ \/ (joins /\ Host # L), !.last = Observe("sync")]
Hear == /\ s.pending # "none"
 /\ s' = [s EXCEPT !.heard = IF ~s.brought \/ Mutation = "hear-data"
                             THEN @ \cup {s.pending} ELSE @,
          !.asked = @ \/ s.pending = s.left,
          !.pending = "none", !.brought = FALSE, !.last = Observe("hear")]
Settle == /\ s.running
 /\ LET allHeard == Sources(s) \subseteq s.heard
        govGive == Host = L /\ Behind(s,"gov") /\
             (NeverShared(s) \/ (Mutation = "empty-shared" /\ Sources(s) = {})
              \/ (Mutation = "one-peer" /\ s.heard # {}))
        govStill == Host = L /\ ((Behind(s,"gov") /\ ~govGive) \/ s.unheard)
        agentGive == Behind(s,"agent") /\ allHeard /\
                     (~govStill \/ Mutation = "give-held")
        memberHeard == Host \in s.heard \/
          ((Sources(s) \ {Host}) # {} /\ (Sources(s) \ {Host}) \subseteq s.heard)
        clearUnknown == s.unheard /\
          (IF Host = L THEN Mutation = "all-listed" /\ allHeard ELSE memberHeard)
    IN /\ govGive \/ agentGive \/ clearUnknown \/ (s.admittedHold /\ Host \in s.heard)
       /\ s' = [s EXCEPT
            !.marks["gov"] = IF govGive THEN Tip(s.hist,"gov",s.db[L]) ELSE @,
            !.marks["agent"] = IF agentGive THEN Tip(s.hist,"agent",s.db[L]) ELSE @,
            !.unheard = @ /\ ~clearUnknown,
            !.admittedHold = @ /\ Host \notin s.heard,
            !.gaveHeld = agentGive /\ govStill, !.last = Observe("settle")]
Continue == /\ AllowContinue /\ s.running /\ ~s.continued
 /\ s' = [s EXCEPT !.marks = [k \in Keys |-> IF k \in Own THEN Tip(s.hist,k,s.db[L]) ELSE 0],
       !.unheard = FALSE, !.admittedHold = FALSE, !.continued = TRUE, !.last = Observe("continue")]

GeneralNext ==
 \/ \E k \in Keys : Sign(k)
 \/ \E p \in Peers \cup {"agent"} : Admit(p)
 \/ \E p \in Peers : Remove(p) \/ Reach(p)
 \/ \E a,b \in Nodes : Sync(a,b)
 \/ Copy \/ LoseMarks \/ Start \/ Hear \/ Settle \/ Continue
 \/ IF RestoreKind = "store" THEN RestoreStore ELSE RestoreAll
(* Ordinary restarts have their own case, avoiding infinite recovery starvation. *)
OrdinaryNext == Restart \/ Start \/ LoseMarks \/ (\E k \in Keys : Sign(k))

(* Directed counterexamples use exactly the same actions as general exploration.
   Entries are commands, not hand-written state assignments. *)
Program == CASE Scenario = "lacking" ->
 <<"copy","g","LQ","store","start","PL","hear","settle","g">>
 [] Scenario = "empty" -> <<"copy","admitQ","LQ","store","start","settle","g">>
 [] Scenario = "rejoin" -> <<"copy","admitA","PL","hear","PL","hear","settle","a","LP","all","start","PL","hear","settle","a">>
 [] Scenario = "give" -> <<"copy","g","a","store","start","PL","hear","settle">>
 [] Scenario = "gate" -> <<"copy","admitQ","store","start","a">>
 [] Scenario = "listed" -> <<"copy","removeP","admitQ","LQ","LP","all","start","PL","hear","settle","g">>
 [] Scenario = "removed" -> <<"copy","a","LP","store","start","removeP","LQ","QL","hear","QL","hear","settle","a">>
 [] Scenario = "member" -> <<"copy","a","LQ","all","start","PL","hear","settle","a">>
 [] Scenario = "private" -> <<"copy","g","store","start","settle","g">>
 [] Scenario = "unseen" -> <<"copy","a","store","start","PL","hear","settle","a">>
 [] Scenario = "recover" -> <<"copy","g","a","LP","store","start">>
 [] Scenario = "removed-keeper" -> <<"copy","removeP","LP","store","start">>
 [] Scenario = "give-recover" -> <<"copy","g","LP","a","store","start">>
 [] Scenario = "alone" -> <<"copy","g","a","store","start">>
 [] Scenario = "override" -> <<"copy","g","LP","all","start","continue","g">>
 [] Scenario = "leave" -> <<"copy","leave","PL","hear","removeP","store","start","PL","hear","continue","removeP">>
 [] OTHER -> <<>>
Command(c) == CASE c = "copy" -> Copy [] c = "g" -> Sign("gov") [] c = "a" -> Sign("agent")
 [] c = "admitQ" -> Admit(Q) [] c = "admitA" -> Admit("agent") [] c = "removeP" -> Remove(P)
 [] c = "leave" -> Leave(P)
 [] c = "LQ" -> Sync(L,Q) [] c = "LP" -> Sync(L,P) [] c = "PL" -> Sync(P,L) [] c = "QL" -> Sync(Q,L)
 [] c = "store" -> RestoreStore [] c = "all" -> RestoreAll [] c = "start" -> Start
 [] c = "hear" -> Hear [] c = "settle" -> Settle [] c = "continue" -> Continue
RecoveryNext == (\E p \in Peers : Reach(p) \/ Sync(p,L)) \/ Hear \/ Settle
Scheduled == IF Scenario = "general" THEN GeneralNext
 ELSE IF Scenario = "ordinary" THEN OrdinaryNext
 ELSE IF phase < Len(Program) THEN Command(Program[phase+1])
 ELSE IF Scenario \in {"recover","alone","removed-keeper","give-recover"} THEN RecoveryNext
 ELSE UNCHANGED s
Next ==
 /\ Scheduled
 /\ phase' = IF Scenario \in {"general","ordinary"} \/ phase >= Len(Program) THEN phase ELSE phase+1
 /\ forkG' = Fork(s'.hist,AllHeld(s'),"gov")
 /\ forkA' = Fork(s'.hist,AllHeld(s'),"agent")
 /\ reusedG' = Fork(s'.hist,1..Len(s'.hist),"gov")
 /\ reusedA' = Fork(s'.hist,1..Len(s'.hist),"agent")
 /\ agentWhileHeld' = (agentWhileHeld \/ s'.signedHeld)
 /\ gaveWhileHeld' = (gaveWhileHeld \/ s'.gaveHeld)
 /\ ordinaryHeld' = (ordinaryHeld \/ s'.ordinaryBad)
 /\ govHeld' = GovHeld(s') /\ agentHeld' = AgentHeld(s')

(* General safety has no fairness. Directed recovery uses strong fairness for
   peer selection and weak fairness for Hear and Settle, never for Sign/Continue.
   General fair cases include eventual Start and peer callback/delivery. *)
Spec == Init /\ [][Next]_vars
(* Reach is independent of the program, including after the restore. Strong
   fairness prevents one pending exchange from starving the other peer. *)
DirectedNext == Next \/
 /\ \E p \in Peers : Reach(p)
 /\ UNCHANGED <<phase,forkG,forkA,reusedG,reusedA,agentWhileHeld,gaveWhileHeld,ordinaryHeld,govHeld,agentHeld>>
DirectedSpec == Init /\ [][DirectedNext]_vars
FairSpec == DirectedSpec /\ WF_vars(DirectedNext)
 /\ (\A p \in Peers : WF_s(Reach(p)) /\ SF_s(Sync(p,L))) /\ WF_s(Hear)
 /\ IF Mutation = "no-settle-fairness" THEN TRUE ELSE WF_s(Settle)

FairGeneralSpec == Spec /\ WF_s(Start)
 /\ (\A p \in Peers : WF_s(Reach(p)) /\ SF_s(Sync(p,L)))
 /\ WF_s(Hear) /\ WF_s(Settle)

TypeOK == /\ s.db \in [Nodes -> SUBSET (1..Len(s.hist))]
 /\ s.marks \in [Keys -> 0..Len(s.hist)] /\ Len(s.hist) <= MaxRecords
 /\ s.heard \subseteq Peers /\ s.reachable \subseteq Peers
 /\ s.restored \in {"none","store","all"}
 /\ <<forkG,forkA,reusedG,reusedA,agentWhileHeld,gaveWhileHeld,ordinaryHeld,govHeld,agentHeld>> \in [1..9 -> BOOLEAN]
NoFork == ~forkG /\ ~forkA
StoreNoFork == s.restored = "store" /\ ~s.continued =>
                  ~forkG /\ (~s.removedAfterCopy => ~forkA)
StoreNoReuse == s.restored = "store" /\ ~s.continued =>
                  ~reusedG /\ (~s.removedAfterCopy => ~reusedA)
AllNoFork == s.restored = "all" /\ Host = L /\ ~s.continued => NoFork
OrdinaryStart == ~ordinaryHeld
AgentFenced == ~agentWhileHeld
NoGiveWhileHeld == ~gaveWhileHeld
AllStayHeld == s.restored = "all" /\ s.running /\ Host = L /\ ~s.continued =>
                GovHeld(s) /\ AgentHeld(s)
RecoveryEligible == s.restored = "store" /\ s.running /\
 (NeverShared(s) \/ s.marks["gov"] \in UNION {s.db[p] : p \in Peers})
HoldsEnd == RecoveryEligible ~> (~GovHeld(s) /\ ~AgentHeld(s))
WillingRecoveryEligible == s.restored = "store" /\ s.running /\
 (NeverShared(s) \/ \E p \in Peers : s.marks["gov"] \in s.db[p] /\
                                      p \in Members(s.hist,s.db[p]))
WillingHoldsEnd == WillingRecoveryEligible ~> (~GovHeld(s) /\ ~AgentHeld(s))
RecoveryReached == <>(s.restored = "store" /\ s.running)
RemovalMatchesBefore ==
  \A i, j \in 1..Len(s.hist) :
    (s.hist[i].kind = "remove" /\ s.hist[j].kind = "remove") =>
      (s.hist[i].key = s.hist[j].key /\
       s.hist[i].pos = s.hist[j].pos /\
       s.hist[i].members = s.hist[j].members /\
       s.hist[i].admitted = s.hist[j].admitted)
(* Witness that the leave trace runs to its end with two removals, so
   RemovalMatchesBefore compares the re-signed removal with the first. *)
TwoRemovals == /\ phase = Len(Program)
 /\ Cardinality({i \in 1..Len(s.hist) : s.hist[i].kind = "remove"}) = 2
NeverTwoRemovals == ~TwoRemovals
=============================================================================
