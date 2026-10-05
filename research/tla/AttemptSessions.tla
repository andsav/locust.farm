-------------------------- MODULE AttemptSessions --------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(***************************************************************************
Current local attempt/session subset. Attempts are independent replicated facts;
a claim is local fencing for ONE attempt, not exclusive ownership of a task or
a distributed lease. The multi-attempt configuration permits two sessions of the
same principal to start separate attempts. Shared eligibility and owner/local
permission are separate inputs. Atomic transactions include the signed event,
claim/session binding and keyed response. Failure may persist all or none;
uncertain failure fences requests until reopening the durable store.

Finite generation/event bounds constrain exploration only. No product limit is
inferred. Membership/round/cancellation changes are supplied as already-verified
external inputs; replicated proof validation belongs to Organization.tla. This
model does not cover signing bytes, sockets, adapters, WAL or physical power loss.
***************************************************************************)
CONSTANTS Principals, Sessions, Attempts, Keys, Worker, Other, A, B, One, Two,
          None, MaxGeneration, MaxEvents, CheckGeneration, Scenario
ASSUME /\ Worker \in Principals /\ Other \in Principals /\ Worker # Other
 /\ A \in Sessions /\ B \in Sessions /\ A # B /\ One \in Attempts
 /\ MaxGeneration >= 1 /\ MaxEvents >= 2
K1 == "k1"
K2 == "k2"
Modes == {"success","before","after"}
EmptyAttempt == [started |-> FALSE, holder |-> None, generation |-> 0,
                 cancelled |-> FALSE, ended |-> FALSE]
EmptyRecord == [attempt |-> None, gen |-> 0, event |-> 0]
EmptyDB == [bindings |-> [s \in Sessions |-> None],
 attempts |-> [a \in Attempts |-> EmptyAttempt], events |-> <<>>,
 idem |-> [p \in Principals |-> [k \in Keys |-> EmptyRecord]],
 member |-> TRUE, departed |-> FALSE, currentRound |-> TRUE,
 localAllowed |-> FALSE, sharedAllowed |-> TRUE, active |-> Principals]
Request(p,s,a,g,k) == [principal |-> p, session |-> s, attempt |-> a, gen |-> g, key |-> k]
Requests == [principal : Principals, session : Sessions, attempt : Attempts,
             gen : 1..MaxGeneration, key : Keys]
EmptyRequest == Request(Worker,A,One,1,K1)

VARIABLES db, mem, failed, pending, lastRequest, lastOutcome, phase, highWater,
 signatureWhileFailed, staleSigned, reopened, witnessReached
vars == <<db,mem,failed,pending,lastRequest,lastOutcome,phase,highWater,
          signatureWhileFailed,staleSigned,reopened,witnessReached>>
Init == /\ db = EmptyDB /\ mem = EmptyDB /\ failed = FALSE /\ pending = None
 /\ lastRequest = EmptyRequest /\ lastOutcome = "Initial" /\ phase = 0
 /\ highWater = [a \in Attempts |-> 0] /\ signatureWhileFailed = FALSE
 /\ staleSigned = FALSE /\ reopened = FALSE /\ witnessReached = FALSE
Advance == phase' = IF Scenario = "safety" THEN phase ELSE phase + 1
Observe(x) == IF Scenario = "safety" THEN "unobserved" ELSE x
Remember(x) == IF Scenario = "safety" THEN EmptyRequest ELSE x
UnchangedClient == UNCHANGED <<pending,lastRequest>>
UnchangedMonitor == UNCHANGED <<signatureWhileFailed,staleSigned,reopened>>
BoundOrFree(d,p,s) == d.bindings[s] \in {None,p}
Current(d,p,a) == p = Worker /\ p \in d.active /\ d.member /\ ~d.departed /\
 d.currentRound /\ d.attempts[a].started /\ ~d.attempts[a].ended
CanStart(d,p,s) == p = Worker /\ p \in d.active /\ d.member /\ ~d.departed /\
 d.currentRound /\ d.localAllowed /\ d.sharedAllowed /\ BoundOrFree(d,p,s)
StaleIn(d) == \E i \in 1..Len(d.events) : d.events[i].kind = "report" /\
 (d.events[i].gen # d.events[i].claimGen \/ d.events[i].session # d.events[i].holder)
Event(kind,p,s,a,g,k,d) == [kind |-> kind, principal |-> p, session |-> s,
 attempt |-> a, gen |-> g, key |-> k, claimGen |-> d.attempts[a].generation,
 holder |-> d.attempts[a].holder, bound |-> BoundOrFree(d,p,s),
 permitted |-> IF kind = "start" THEN CanStart(d,p,s)
              ELSE Current(d,p,a) /\ ~d.attempts[a].cancelled]
Commit(next,mode,authors) ==
 /\ mode \in Modes /\ db' = IF mode = "before" THEN db ELSE next
 /\ mem' = IF mode = "success" THEN next ELSE mem
 /\ failed' = (mode # "success")
 /\ highWater' = IF mode = "before" THEN highWater ELSE
       [a \in Attempts |-> IF next.attempts[a].generation > highWater[a]
        THEN next.attempts[a].generation ELSE highWater[a]]
 /\ signatureWhileFailed' = (signatureWhileFailed \/ (authors /\ failed))
 /\ staleSigned' = (staleSigned \/ (authors /\ StaleIn(next)))
 /\ UNCHANGED reopened

Permission(value) == /\ ~failed /\ value \in BOOLEAN /\ mem.localAllowed # value
 /\ Commit([mem EXCEPT !.localAllowed = value],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("PermissionChanged")
Eligibility(value) == /\ ~failed /\ value \in BOOLEAN /\ mem.sharedAllowed # value
 /\ Commit([mem EXCEPT !.sharedAllowed = value],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("EligibilityChanged")
Bind(p,s) == /\ ~failed /\ p \in mem.active /\ s \in Sessions /\ mem.bindings[s] = None
 /\ Commit([mem EXCEPT !.bindings[s] = p],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("Bound")
Start(p,s,a,mode) ==
 /\ ~failed /\ CanStart(mem,p,s) /\ a \in Attempts /\ ~mem.attempts[a].started
 /\ ~\E other \in Attempts : Current(mem,p,other) /\ mem.attempts[other].holder = s
 /\ Len(mem.events) < MaxEvents
 /\ LET next == [mem EXCEPT !.bindings[s] = p,
       !.attempts[a] = [started |-> TRUE, holder |-> s, generation |-> 1,
                       cancelled |-> FALSE, ended |-> FALSE],
       !.events = Append(@,Event("start",p,s,a,1,None,mem))]
    IN Commit(next,mode,TRUE)
 /\ UnchangedClient /\ Advance
 /\ lastOutcome' = Observe(IF mode = "success" THEN "Started" ELSE "Internal")
StartDenied(p,s,a) == /\ ~failed /\ a \in Attempts /\ ~mem.attempts[a].started
 /\ ~CanStart(mem,p,s)
 /\ UNCHANGED <<db,mem,failed,highWater>> /\ UnchangedClient /\ UnchangedMonitor
 /\ Advance /\ lastOutcome' = Observe("Denied")
Takeover(p,s,a,mode) == /\ ~failed /\ Current(mem,p,a) /\ BoundOrFree(mem,p,s)
 /\ mem.localAllowed /\ mem.attempts[a].holder # s
 /\ mem.attempts[a].generation < MaxGeneration
 /\ Commit([mem EXCEPT !.bindings[s] = p, !.attempts[a].holder = s,
       !.attempts[a].generation = @ + 1],mode,FALSE)
 /\ UnchangedClient /\ Advance
 /\ lastOutcome' = Observe(IF mode = "success" THEN "TakenOver" ELSE "Internal")
Queue(p,s,a,g,k) == /\ pending = None /\ Request(p,s,a,g,k) \in Requests
 /\ pending' = Request(p,s,a,g,k)
 /\ UNCHANGED <<db,mem,failed,lastRequest,highWater,signatureWhileFailed,staleSigned,reopened>>
 /\ Advance /\ lastOutcome' = Observe("Queued")
Retry == /\ pending = None /\ pending' = lastRequest
 /\ UNCHANGED <<db,mem,failed,lastRequest,highWater,signatureWhileFailed,staleSigned,reopened>>
 /\ Advance /\ lastOutcome' = Observe("Queued")
Reject(reason) == /\ pending' = None /\ lastRequest' = Remember(pending)
 /\ UNCHANGED <<db,mem,failed,highWater>> /\ UnchangedMonitor
 /\ Advance /\ lastOutcome' = Observe(reason)
Handle(mode) == /\ pending # None /\ mode \in Modes
 /\ LET r == pending
        saved == db.idem[r.principal][r.key]
        holds == mem.attempts[r.attempt].holder = r.session /\
                 (~CheckGeneration \/ mem.attempts[r.attempt].generation = r.gen)
    IN CASE failed -> Reject("Internal")
       [] r.principal \notin mem.active -> Reject("Denied")
       [] saved.event # 0 -> Reject(IF saved.attempt = r.attempt /\ saved.gen = r.gen
            THEN "Replayed" ELSE "IdempotencyMismatch")
       [] ~Current(mem,r.principal,r.attempt) -> Reject("Denied")
       [] ~holds -> Reject("Superseded")
       [] mem.attempts[r.attempt].cancelled -> Reject("Conflict")
       [] OTHER ->
          /\ Len(mem.events) < MaxEvents
          /\ LET next == [mem EXCEPT
                 !.events = Append(@,Event("report",r.principal,r.session,r.attempt,r.gen,r.key,mem)),
                 !.idem[r.principal][r.key] = [attempt |-> r.attempt, gen |-> r.gen,
                                               event |-> Len(mem.events) + 1]]
             IN Commit(next,mode,TRUE)
          /\ pending' = None /\ lastRequest' = Remember(r) /\ Advance
          /\ lastOutcome' = Observe(IF mode = "success" THEN "Recorded" ELSE "Internal")
Cancel(a) == /\ ~failed /\ mem.attempts[a].started /\ ~mem.attempts[a].cancelled
 /\ Commit([mem EXCEPT !.attempts[a].cancelled = TRUE],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("Cancelled")
(* Abstract the atomic stopped acknowledgment plus terminal report. The two
    signed records and their replication are checked by the Rust tests. *)
AcknowledgeStopped(p,s,a,g) ==
 /\ ~failed /\ Current(mem,p,a) /\ mem.attempts[a].cancelled
 /\ mem.attempts[a].holder = s /\ mem.attempts[a].generation = g
 /\ Commit([mem EXCEPT !.attempts[a].ended = TRUE],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("Stopped")
End(a) == /\ ~failed /\ mem.attempts[a].started /\ ~mem.attempts[a].ended
 /\ Commit([mem EXCEPT !.attempts[a].ended = TRUE],"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("Ended")
Fence(kind) == /\ ~failed /\ kind \in {"remove","leave","revise"}
 /\ LET next == CASE kind = "remove" -> [mem EXCEPT !.member = FALSE]
                 [] kind = "leave" -> [mem EXCEPT !.departed = TRUE]
                 [] OTHER -> [mem EXCEPT !.currentRound = FALSE]
    IN /\ next # mem /\ Commit(next,"success",FALSE)
 /\ UnchangedClient /\ Advance /\ lastOutcome' = Observe("Fenced")
Reopen == /\ (failed \/ ~reopened) /\ mem' = db /\ failed' = FALSE
 /\ reopened' = (Scenario # "safety")
 /\ UNCHANGED <<db,pending,lastRequest,highWater,signatureWhileFailed,staleSigned>>
 /\ Advance /\ lastOutcome' = Observe("Reopened")
Quiesce == UNCHANGED <<db,mem,failed,pending,lastRequest,lastOutcome,phase,highWater,
                       signatureWhileFailed,staleSigned,reopened>>
SafetyNext ==
 \/ \E v \in BOOLEAN : Permission(v) \/ Eligibility(v)
 \/ \E p \in Principals,s \in Sessions : Bind(p,s)
 \/ \E s \in Sessions,a \in Attempts,m \in Modes : Start(Worker,s,a,m) \/ Takeover(Worker,s,a,m)
 \/ \E p \in Principals,s \in Sessions,a \in Attempts,g \in 1..MaxGeneration,k \in Keys : Queue(p,s,a,g,k)
 \/ \E m \in Modes : Handle(m)
 \/ \E a \in Attempts : Cancel(a) \/ End(a)
 \/ \E s \in Sessions,a \in Attempts,g \in 1..MaxGeneration : AcknowledgeStopped(Worker,s,a,g)
 \/ \E kind \in {"remove","leave","revise"} : Fence(kind)
 \/ Reopen \/ Quiesce
ABANext ==
 \/ /\ phase = 0 /\ Permission(TRUE)
 \/ /\ phase = 1 /\ Start(Worker,A,One,"success")
 \/ /\ phase = 2 /\ Queue(Worker,A,One,1,K1)
 \/ /\ phase = 3 /\ Takeover(Worker,B,One,"success")
 \/ /\ phase = 4 /\ Takeover(Worker,A,One,"success")
 \/ /\ phase = 5 /\ Handle("success")
 \/ /\ phase = 6 /\ Quiesce
ReplayNext ==
 \/ /\ phase = 0 /\ Permission(TRUE)
 \/ /\ phase = 1 /\ Start(Worker,A,One,"success")
 \/ /\ phase = 2 /\ Queue(Worker,A,One,1,K1)
 \/ /\ phase = 3 /\ Handle("success")
 \/ /\ phase = 4 /\ Takeover(Worker,B,One,"success")
 \/ /\ phase = 5 /\ Takeover(Worker,A,One,"success")
 \/ /\ phase = 6 /\ Reopen
 \/ /\ phase = 7 /\ Retry
 \/ /\ phase = 8 /\ Handle("success")
 \/ /\ phase = 9 /\ Quiesce
FailureNext(mode) ==
 \/ /\ phase = 0 /\ Permission(TRUE)
 \/ /\ phase = 1 /\ Start(Worker,A,One,"success")
 \/ /\ phase = 2 /\ Queue(Worker,A,One,1,K1)
 \/ /\ phase = 3 /\ Handle(mode)
 \/ /\ phase = 4 /\ Retry
 \/ /\ phase = 5 /\ Handle("success")
 \/ /\ phase = 6 /\ Reopen
 \/ /\ phase = 7 /\ Retry
 \/ /\ phase = 8 /\ Handle("success")
 \/ /\ phase = 9 /\ Quiesce
CancelNext ==
 \/ /\ phase = 0 /\ Permission(TRUE)
 \/ /\ phase = 1 /\ Start(Worker,A,One,"success")
 \/ /\ phase = 2 /\ Queue(Worker,A,One,1,K1)
 \/ /\ phase = 3 /\ Cancel(One)
 \/ /\ phase = 4 /\ Handle("success")
 \/ /\ phase = 5 /\ lastOutcome = "Conflict" /\ AcknowledgeStopped(Worker,A,One,1)
 \/ /\ phase = 6 /\ Reopen
 \/ /\ phase = 7 /\ Quiesce
PrincipalNext ==
 \/ /\ phase = 0 /\ Bind(Other,A)
 \/ /\ phase = 1 /\ Permission(TRUE)
 \/ /\ phase = 2 /\ StartDenied(Worker,A,One)
 \/ /\ phase = 3 /\ Quiesce
IndependentNext ==
 \/ /\ phase = 0 /\ Permission(TRUE)
 \/ /\ phase = 1 /\ Start(Worker,A,One,"success")
 \/ /\ phase = 2 /\ Start(Worker,B,Two,"success")
 \/ /\ phase = 3 /\ Quiesce
PermissionNext ==
 \/ /\ phase = 0 /\ StartDenied(Worker,A,One)
 \/ /\ phase = 1 /\ Permission(TRUE)
 \/ /\ phase = 2 /\ Eligibility(FALSE)
 \/ /\ phase = 3 /\ StartDenied(Worker,A,One)
 \/ /\ phase = 4 /\ Quiesce
ScheduledNext == CASE Scenario = "safety" -> SafetyNext
 [] Scenario = "aba" -> ABANext [] Scenario = "replay" -> ReplayNext
 [] Scenario = "failure-before" -> FailureNext("before")
 [] Scenario = "failure-after" -> FailureNext("after")
 [] Scenario = "cancel" -> CancelNext [] Scenario = "principal" -> PrincipalNext
 [] Scenario = "independent" -> IndependentNext [] Scenario = "permissions" -> PermissionNext
Witness == CASE Scenario = "aba" -> phase = 6 /\ lastOutcome = "Superseded" /\ Len(db.events) = 1
 [] Scenario = "replay" -> phase = 9 /\ reopened /\ lastOutcome = "Replayed" /\ Len(db.events) = 2
 [] Scenario = "failure-before" -> phase = 9 /\ reopened /\ lastOutcome = "Recorded" /\ Len(db.events) = 2
 [] Scenario = "failure-after" -> phase = 9 /\ reopened /\ lastOutcome = "Replayed" /\ Len(db.events) = 2
 [] Scenario = "cancel" -> phase = 7 /\ reopened /\ db.attempts[One].ended /\ Len(db.events) = 1
 [] Scenario = "principal" -> phase = 3 /\ lastOutcome = "Denied" /\ Len(db.events) = 0
 [] Scenario = "independent" -> phase = 3 /\ Len(db.events) = 2 /\
       db.attempts[One].holder = A /\ db.attempts[Two].holder = B
 [] Scenario = "permissions" -> phase = 4 /\ lastOutcome = "Denied" /\ Len(db.events) = 0
 [] OTHER -> FALSE
Next == /\ ScheduledNext /\ witnessReached' = Witness'
Spec == Init /\ [][Next]_vars

TypeOK == /\ failed \in BOOLEAN /\ pending \in Requests \cup {None}
 /\ lastRequest \in Requests /\ Len(db.events) <= MaxEvents /\ Len(mem.events) <= MaxEvents
 /\ phase \in 0..9 /\ witnessReached \in BOOLEAN /\ staleSigned \in BOOLEAN
 /\ \A a \in Attempts : db.attempts[a].generation \in 0..MaxGeneration
ClaimHolderBound == \A a \in Attempts : db.attempts[a].started => db.bindings[db.attempts[a].holder] = Worker
GenerationsNeverDecrease == \A a \in Attempts : db.attempts[a].generation >= highWater[a]
NoCrossPrincipalEvents == \A i \in 1..Len(db.events) : db.events[i].principal = Worker /\ db.events[i].bound
StaleWritesCannotAuthor == ~staleSigned /\ ~StaleIn(db)
OnlyAuthorizedWrites == \A i \in 1..Len(db.events) : db.events[i].permitted
IdempotentRequestsAuthorOnce == \A i,j \in 1..Len(db.events) :
 (db.events[i].key # None /\ db.events[j].key # None /\
  db.events[i].principal = db.events[j].principal /\ db.events[i].key = db.events[j].key) => i = j
IdempotencyMatchesEvent == \A p \in Principals,k \in Keys :
 LET r == db.idem[p][k] IN r.event # 0 => r.event <= Len(db.events) /\
 db.events[r.event].principal = p /\ db.events[r.event].key = k /\
 db.events[r.event].attempt = r.attempt /\ db.events[r.event].gen = r.gen
UncertainCommitFencesSigning == ~signatureWhileFailed
HealthyMemoryMatchesStore == ~failed => mem = db
NeverWitness == ~witnessReached
=============================================================================
