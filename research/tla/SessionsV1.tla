----------------------------- MODULE SessionsV1 -----------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(***************************************************************************
Protocol/API version-1 local-daemon claim/write subset; this is not a distributed lease model.

One assignment and one enrolled assignee are sufficient to expose A-B-A
takeover, principal/session binding, caller-scoped idempotency, and uncertain
atomic commit outcomes. db is the durable store; mem is the daemon's view.
On an uncertain failure, db may contain all or none of the transaction while
mem stays unchanged. failed forbids further request planning until Reopen.

Safety checking quotients out request/outcome diagnostics, which do not affect
request planning; directed witnesses retain those diagnostics.

The atomic transaction assumption deliberately omits WAL/page-cache/flush and
power-loss behavior. Those belong to the later Durability model. Successful
transactions commit events, session binding and idempotency records together.

Local departure is a durable external input for the worker. Canonical membership
is an abstract Boolean supplied by goal replay. This model does not include
invitations, content read epochs, coordinator self-removal, halted/incomplete
goals, shutdown requests, blob staging, socket cleanup or power loss. The claim
fixture assumes a usable, non-halted goal and available signing/content keys.
Only progress requests carry idempotency keys here; replay after departure may
return a previous success but cannot sign a new event.

Source map: node/{sessions,access,commit,authoring,callers}.rs and
node/requests/{claims,tasks,mod}.rs. The request digest contains the request
generation but does not contain the connection's session, matching Rust.
An identical retry is replayed BEFORE claim-generation/member checks; it may
return an old response after takeover without authoring a new event.
***************************************************************************)

CONSTANTS Principals, Sessions, Assignee, A, B, Keys, None,
          MaxGeneration, MaxEvents, CheckGeneration, Scenario

ASSUME /\ Assignee \in Principals
       /\ A \in Sessions /\ B \in Sessions /\ A # B
       /\ MaxGeneration >= 3 /\ MaxEvents >= 2
       /\ CheckGeneration \in BOOLEAN
       /\ Cardinality(Keys) >= 2

Other == CHOOSE p \in Principals : p # Assignee
K1 == CHOOSE k \in Keys : k = "k1"
K2 == CHOOSE k \in Keys : k = "k2"
CommitModes == {"success", "before", "after"}

EmptyRecord == [gen |-> 0, event |-> 0]
EmptyDB == [bindings |-> [s \in Sessions |-> None],
            active |-> Principals,
            member |-> TRUE, departed |-> FALSE,
            grants |-> [claim |-> FALSE, takeover |-> FALSE],
            authorized |-> [claim |-> FALSE, takeover |-> FALSE],
            current |-> TRUE, revoked |-> FALSE, cancelled |-> FALSE,
            accepted |-> FALSE, taskState |-> "Assigned",
            holder |-> None, generation |-> 0,
            events |-> <<>>,
            idem |-> [p \in Principals |-> [k \in Keys |-> EmptyRecord]]]

Request(p, s, g, k) == [principal |-> p, session |-> s, gen |-> g, key |-> k]
RequestSet == [principal : Principals, session : Sessions,
               gen : 1..MaxGeneration, key : Keys]
EmptyRequest == Request(Assignee, A, 1, K1)

VARIABLES db, mem, failed, pending, lastRequest, lastOutcome, lastAction,
          highWater, signatureWhileFailed, reopened, phase,
          witnessReached, staleWriteAuthored, staleWriteSigned

vars == <<db, mem, failed, pending, lastRequest, lastOutcome, lastAction,
          highWater, signatureWhileFailed, reopened, phase,
          witnessReached, staleWriteAuthored, staleWriteSigned>>

Init == /\ db = EmptyDB /\ mem = EmptyDB /\ failed = FALSE
        /\ pending = None /\ lastRequest = EmptyRequest
        /\ lastOutcome = "Initial" /\ lastAction = "Init"
        /\ highWater = 0 /\ signatureWhileFailed = FALSE
        /\ reopened = FALSE /\ phase = 0
        /\ witnessReached = FALSE /\ staleWriteAuthored = FALSE
        /\ staleWriteSigned = FALSE

Advance == phase' = IF Scenario = "safety" THEN phase ELSE phase + 1
Observe(value) == IF Scenario = "safety" THEN "unobserved" ELSE value
ObserveRequest(value) == IF Scenario = "safety" THEN EmptyRequest ELSE value

UnchangedClient == UNCHANGED <<pending, lastRequest>>
UnchangedMonitor == UNCHANGED <<signatureWhileFailed, reopened, staleWriteSigned>>

Current(d, p) == /\ p = Assignee /\ d.member /\ ~d.departed
                /\ d.current /\ ~d.revoked /\ ~d.accepted
BoundOrFree(d, p, s) == d.bindings[s] \in {None, p}
CanClaim(d) == d.grants.claim \/ d.authorized.claim
CanTakeover(d) == d.grants.takeover \/ d.authorized.takeover

StaleIn(d) ==
    \E i \in 1..Len(d.events) :
        d.events[i].kind = "progress" /\
        (d.events[i].gen # d.events[i].claimGen \/
         d.events[i].session # d.events[i].holder)

(* Atomic commit is the same action for all planned requests. Failure is not
   interpreted as evidence of rollback. No response/event is released before
   success, and failure fences every later request, including reads/retries. *)
Commit(next, mode, authors) ==
    /\ mode \in CommitModes
    /\ db' = IF mode = "before" THEN db ELSE next
    /\ mem' = IF mode = "success" THEN next ELSE mem
    /\ failed' = (mode # "success")
    /\ highWater' = IF mode = "before" THEN highWater
                    ELSE IF next.generation > highWater
                         THEN next.generation ELSE highWater
    /\ signatureWhileFailed' = (signatureWhileFailed \/ (authors /\ failed))
    /\ staleWriteSigned' = (staleWriteSigned \/ (authors /\ StaleIn(next)))
    /\ UNCHANGED reopened

Event(kind, p, s, g, d, k) ==
    [kind |-> kind, principal |-> p, session |-> s, gen |-> g,
     claimGen |-> d.generation, holder |-> d.holder, key |-> k,
     permitted |-> Current(d, p) /\ ~d.cancelled /\ p \in d.active,
     bound |-> BoundOrFree(d, p, s)]

OwnerAuthorize(takeover) ==
    /\ ~failed /\ takeover \in BOOLEAN
    /\ ~mem.authorized.claim \/ (takeover /\ ~mem.authorized.takeover)
    /\ LET next == [mem EXCEPT !.authorized =
                      [claim |-> TRUE, takeover |-> @.takeover \/ takeover]]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("OwnerAuthorize") /\ lastOutcome' = Observe("Authorized")

SetGrants(claim, takeover) ==
    /\ ~failed /\ claim \in BOOLEAN /\ takeover \in BOOLEAN
    /\ mem.grants # [claim |-> claim, takeover |-> takeover]
    /\ LET next == [mem EXCEPT !.grants = [claim |-> claim, takeover |-> takeover]]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("SetGrants") /\ lastOutcome' = Observe("Granted")

Bind(p, s) ==
    /\ ~failed /\ p \in mem.active /\ s \in Sessions
    /\ mem.bindings[s] = None
    /\ LET next == [mem EXCEPT !.bindings[s] = p]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Bind") /\ lastOutcome' = Observe("Bound")

Claim(p, s, mode) ==
    /\ ~failed /\ p \in mem.active /\ s \in Sessions
    /\ Current(mem, p) /\ mem.holder = None
    /\ mem.taskState = "Assigned" /\ CanClaim(mem)
    /\ BoundOrFree(mem, p, s) /\ Len(mem.events) < MaxEvents
    /\ LET next == [mem EXCEPT !.bindings[s] = p,
                      !.holder = s, !.generation = 1, !.taskState = "Taken",
                      !.events = Append(@, Event("claim", p, s, 1, mem, None))]
       IN Commit(next, mode, TRUE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Claim")
    /\ lastOutcome' = Observe(IF mode = "success" THEN "Claimed" ELSE "Internal")

ClaimDenied(p, s) ==
    /\ ~failed /\ p \in mem.active /\ Current(mem, p)
    /\ s \in Sessions /\ mem.holder = None /\ CanClaim(mem)
    /\ mem.taskState = "Assigned"
    /\ ~BoundOrFree(mem, p, s)
    /\ UNCHANGED <<db, mem, failed, highWater>>
    /\ UnchangedClient /\ UnchangedMonitor /\ Advance
    /\ lastAction' = Observe("Claim") /\ lastOutcome' = Observe("Denied")

RecoverClaim(p, s) ==
    /\ ~failed /\ p \in mem.active /\ Current(mem, p)
    /\ mem.holder = s /\ s \in Sessions
    /\ UNCHANGED <<db, mem, failed, highWater>>
    /\ UnchangedClient /\ UnchangedMonitor /\ Advance
    /\ lastAction' = Observe("RecoverClaim") /\ lastOutcome' = Observe("Claimed")

Takeover(p, s, mode) ==
    /\ ~failed /\ p \in mem.active /\ Current(mem, p)
    /\ s \in Sessions /\ mem.holder \in Sessions /\ mem.holder # s
    /\ CanTakeover(mem) /\ BoundOrFree(mem, p, s)
    /\ mem.generation < MaxGeneration
    /\ LET next == [mem EXCEPT !.bindings[s] = p, !.holder = s,
                      !.generation = @ + 1]
       IN Commit(next, mode, FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Takeover")
    /\ lastOutcome' = Observe(IF mode = "success" THEN "Claimed" ELSE "Internal")

(* A client can issue/delay any claimed-generation request. The node processes
   it later against its then-current session, assignment and credential state. *)
Queue(p, s, g, k) ==
    /\ pending = None /\ Request(p, s, g, k) \in RequestSet
    /\ pending' = Request(p, s, g, k)
    /\ UNCHANGED <<db, mem, failed, lastRequest, highWater,
                   signatureWhileFailed, reopened, staleWriteSigned>>
    /\ Advance /\ lastAction' = Observe("Queue") /\ lastOutcome' = Observe("Queued")

Retry ==
    /\ pending = None /\ pending' = lastRequest
    /\ UNCHANGED <<db, mem, failed, lastRequest, highWater,
                   signatureWhileFailed, reopened, staleWriteSigned>>
    /\ Advance /\ lastAction' = Observe("Retry") /\ lastOutcome' = Observe("Queued")

Rejected(reason) ==
    /\ pending # None /\ pending' = None /\ lastRequest' = ObserveRequest(pending)
    /\ UNCHANGED <<db, mem, failed, highWater>>
    /\ UnchangedMonitor /\ Advance
    /\ lastAction' = Observe("Handle") /\ lastOutcome' = Observe(reason)

Handle(mode) ==
    /\ pending # None /\ mode \in CommitModes
    /\ LET r == pending
           recorded == db.idem[r.principal][r.key]
           holds == mem.holder = r.session /\
                    (~CheckGeneration \/ mem.generation = r.gen)
       IN CASE failed -> Rejected("Internal")
          [] r.principal \notin mem.active -> Rejected("Denied")
          [] recorded.event # 0 ->
               Rejected(IF recorded.gen = r.gen THEN "Replayed"
                        ELSE "IdempotencyMismatch")
          [] r.principal # Assignee \/ ~mem.member \/ mem.departed -> Rejected("Denied")
          [] ~Current(mem, r.principal) -> Rejected("Superseded")
          [] ~holds -> Rejected("Superseded")
          [] mem.cancelled \/ mem.taskState # "Taken" -> Rejected("Conflict")
          [] OTHER ->
               /\ Len(mem.events) < MaxEvents
               /\ LET next == [mem EXCEPT
                         !.events = Append(@, Event("progress", r.principal,
                                             r.session, r.gen, mem, r.key)),
                         !.idem[r.principal][r.key] =
                           [gen |-> r.gen, event |-> Len(mem.events) + 1]]
                  IN Commit(next, mode, TRUE)
               /\ pending' = None /\ lastRequest' = ObserveRequest(r) /\ Advance
               /\ lastAction' = Observe("Handle")
               /\ lastOutcome' = Observe(IF mode = "success" THEN "Recorded" ELSE "Internal")

Cancel ==
    /\ ~failed /\ mem.current /\ ~mem.cancelled /\ ~mem.accepted
    /\ LET next == [mem EXCEPT !.cancelled = TRUE, !.taskState = "CancelRequested"]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Cancel") /\ lastOutcome' = Observe("Cancelled")

ChangeAssignment(reason) ==
    /\ ~failed /\ reason \in {"revoked", "superseded", "accepted", "ended"}
    /\ mem.current /\ ~mem.revoked /\ ~mem.accepted /\ mem.taskState # "Ended"
    /\ LET next == CASE reason = "revoked" -> [mem EXCEPT !.revoked = TRUE]
                   [] reason = "superseded" -> [mem EXCEPT !.current = FALSE]
                   [] reason = "accepted" -> [mem EXCEPT !.accepted = TRUE]
                   [] OTHER -> [mem EXCEPT !.taskState = "Ended"]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("ChangeAssignment") /\ lastOutcome' = Observe(reason)

Revoke(p) ==
    /\ ~failed /\ p \in mem.active
    /\ LET next == [mem EXCEPT !.active = @ \ {p}]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Revoke") /\ lastOutcome' = Observe("Revoked")

RemoveMember ==
    /\ ~failed /\ mem.member
    /\ LET next == [mem EXCEPT !.member = FALSE]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("RemoveMember") /\ lastOutcome' = Observe("Removed")

Leave ==
    /\ ~failed /\ ~mem.departed
    /\ LET next == [mem EXCEPT !.departed = TRUE]
       IN Commit(next, "success", FALSE)
    /\ UnchangedClient /\ Advance
    /\ lastAction' = Observe("Leave") /\ lastOutcome' = Observe("Left")

Reopen ==
    /\ failed \/ ~reopened
    /\ mem' = db /\ failed' = FALSE /\ reopened' = (Scenario # "safety")
    /\ UNCHANGED <<db, pending, lastRequest, highWater, signatureWhileFailed,
                   staleWriteSigned>>
    /\ Advance /\ lastAction' = Observe("Reopen") /\ lastOutcome' = Observe("Reopened")

(* Quiescence is intentional only when there is no pending client request.
   This explicit terminal action retains deadlock detection for stuck requests. *)
Quiesce == /\ pending = None /\ UNCHANGED vars

SafetyNext ==
    \/ \E t \in BOOLEAN : OwnerAuthorize(t)
    \/ \E c, t \in BOOLEAN : SetGrants(c, t)
    \/ \E p \in Principals, s \in Sessions : Bind(p, s)
    \/ \E p \in Principals, s \in Sessions, m \in CommitModes : Claim(p, s, m)
    \/ \E p \in Principals, s \in Sessions : ClaimDenied(p, s)
    \/ \E p \in Principals, s \in Sessions : RecoverClaim(p, s)
    \/ \E p \in Principals, s \in Sessions, m \in CommitModes : Takeover(p, s, m)
    \/ \E p \in Principals, s \in Sessions, g \in 1..MaxGeneration,
          k \in Keys : Queue(p, s, g, k)
    \/ \E m \in CommitModes : Handle(m)
    \/ Cancel
    \/ \E r \in {"revoked", "superseded", "accepted", "ended"} : ChangeAssignment(r)
    \/ \E p \in Principals : Revoke(p)
    \/ RemoveMember \/ Leave \/ Reopen \/ Quiesce

(* Directed witness configurations use the SAME action definitions. Restricting
   the schedule here is for short auditable reachability/mutation traces; the
   safety configuration above checks arbitrary action order in its bounds. *)
ABANext ==
    \/ /\ phase = 0 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 1 /\ Claim(Assignee, A, "success")
    \/ /\ phase = 2 /\ Queue(Assignee, A, 1, K1)
    \/ /\ phase = 3 /\ Takeover(Assignee, B, "success")
    \/ /\ phase = 4 /\ Takeover(Assignee, A, "success")
    \/ /\ phase = 5 /\ Handle("success")
    \/ /\ phase = 6 /\ Quiesce

ReplayNext ==
    \/ /\ phase = 0 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 1 /\ Claim(Assignee, A, "success")
    \/ /\ phase = 2 /\ Queue(Assignee, A, 1, K1)
    \/ /\ phase = 3 /\ Handle("success")
    \/ /\ phase = 4 /\ Takeover(Assignee, B, "success")
    \/ /\ phase = 5 /\ Takeover(Assignee, A, "success")
    \/ /\ phase = 6 /\ Reopen
    \/ /\ phase = 7 /\ Retry
    \/ /\ phase = 8 /\ Handle("success")
    \/ /\ phase = 9 /\ Quiesce

FailureNext(mode) ==
    \/ /\ phase = 0 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 1 /\ Claim(Assignee, A, "success")
    \/ /\ phase = 2 /\ Queue(Assignee, A, 1, K1)
    \/ /\ phase = 3 /\ Handle(mode)
    \/ /\ phase = 4 /\ Retry
    \/ /\ phase = 5 /\ Handle("success")
    \/ /\ phase = 6 /\ Reopen
    \/ /\ phase = 7 /\ Retry
    \/ /\ phase = 8 /\ Handle("success")
    \/ /\ phase = 9 /\ Quiesce

CancelNext ==
    \/ /\ phase = 0 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 1 /\ Claim(Assignee, A, "success")
    \/ /\ phase = 2 /\ Queue(Assignee, A, 1, K1)
    \/ /\ phase = 3 /\ Cancel
    \/ /\ phase = 4 /\ Handle("success")
    \/ /\ phase = 5 /\ Quiesce

AuthorityNext ==
    \/ /\ phase = 0 /\ Bind(Other, A)
    \/ /\ phase = 1 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 2 /\ ClaimDenied(Assignee, A)
    \/ /\ phase = 3 /\ Quiesce

DepartureNext(replay) ==
    \/ /\ phase = 0 /\ OwnerAuthorize(TRUE)
    \/ /\ phase = 1 /\ Claim(Assignee, A, "success")
    \/ /\ phase = 2 /\ Queue(Assignee, A, 1, K1)
    \/ /\ phase = 3 /\ (IF replay THEN Handle("success") ELSE Leave)
    \/ /\ phase = 4 /\ (IF replay THEN Leave ELSE Handle("success"))
    \/ /\ phase = 5 /\ (IF replay THEN Retry ELSE Reopen)
    \/ /\ phase = 6 /\ (IF replay THEN Handle("success") ELSE Queue(Assignee, A, 1, K2))
    \/ /\ phase = 7 /\ (IF replay THEN Reopen ELSE Handle("success"))
    \/ /\ phase = 8 /\ Quiesce

ScheduledNext == CASE Scenario = "safety" -> SafetyNext
        [] Scenario = "aba" -> ABANext
        [] Scenario = "replay" -> ReplayNext
        [] Scenario = "failure-before" -> FailureNext("before")
        [] Scenario = "failure-after" -> FailureNext("after")
        [] Scenario = "cancel" -> CancelNext
        [] Scenario = "authority" -> AuthorityNext
        [] Scenario = "departure" -> DepartureNext(FALSE)
        [] Scenario = "departure-replay" -> DepartureNext(TRUE)

Reached(d, outcome, request, step, restarted, broken) ==
    CASE Scenario = "aba" ->
           step = 6 /\ outcome = "Superseded" /\ d.holder = A /\
           d.generation = 3 /\ request.session = A /\ request.gen = 1 /\
           Len(d.events) = 1
    [] Scenario = "replay" ->
           step = 9 /\ restarted /\ outcome = "Replayed" /\
           d.holder = A /\ d.generation = 3 /\ Len(d.events) = 2
    [] Scenario = "failure-before" ->
           step = 9 /\ restarted /\ ~broken /\ outcome = "Recorded" /\
           d.holder = A /\ d.generation = 1 /\ Len(d.events) = 2
    [] Scenario = "failure-after" ->
           step = 9 /\ restarted /\ ~broken /\ outcome = "Replayed" /\
           d.holder = A /\ d.generation = 1 /\ Len(d.events) = 2
    [] Scenario = "cancel" ->
           step = 5 /\ d.cancelled /\ outcome = "Conflict" /\ Len(d.events) = 1
    [] Scenario = "authority" ->
           step = 3 /\ d.bindings[A] = Other /\ outcome = "Denied" /\
           Len(d.events) = 0
    [] OTHER -> FALSE

Next == /\ ScheduledNext
        /\ witnessReached' = Reached(db', lastOutcome', lastRequest', phase',
                                     reopened', failed')
        /\ staleWriteAuthored' = StaleIn(db')

Spec == Init /\ [][Next]_vars

DBType ==
    [bindings : [Sessions -> Principals \cup {None}], active : SUBSET Principals,
     member : BOOLEAN, departed : BOOLEAN, grants : [claim : BOOLEAN, takeover : BOOLEAN],
     authorized : [claim : BOOLEAN, takeover : BOOLEAN],
     current : BOOLEAN, revoked : BOOLEAN, cancelled : BOOLEAN, accepted : BOOLEAN,
     taskState : {"Assigned", "Taken", "CancelRequested", "Ended"},
     holder : Sessions \cup {None}, generation : 0..MaxGeneration,
     events : Seq([kind : {"claim", "progress"}, principal : Principals,
                   session : Sessions, gen : 1..MaxGeneration,
                   claimGen : 0..MaxGeneration, holder : Sessions \cup {None},
                   key : Keys \cup {None}, permitted : BOOLEAN, bound : BOOLEAN]),
     idem : [Principals -> [Keys -> [gen : 0..MaxGeneration, event : 0..MaxEvents]]]]

TypeOK == /\ db \in DBType /\ mem \in DBType
          /\ Len(db.events) <= MaxEvents /\ Len(mem.events) <= MaxEvents
          /\ failed \in BOOLEAN /\ pending \in RequestSet \cup {None}
          /\ lastRequest \in RequestSet /\ highWater \in 0..MaxGeneration
          /\ signatureWhileFailed \in BOOLEAN /\ reopened \in BOOLEAN
          /\ witnessReached \in BOOLEAN /\ staleWriteAuthored \in BOOLEAN
          /\ staleWriteSigned \in BOOLEAN
          /\ phase \in 0..9

ClaimHolderBound == db.holder = None \/ db.bindings[db.holder] = Assignee
GenerationsNeverDecrease == db.generation >= highWater
NoCrossPrincipalEvents ==
    \A i \in 1..Len(db.events) :
        /\ db.events[i].principal = Assignee
        /\ db.bindings[db.events[i].session] = db.events[i].principal
        /\ db.events[i].bound
StaleWritesCannotAuthor ==
    /\ ~staleWriteSigned
    /\ \A i \in 1..Len(db.events) :
        db.events[i].kind = "progress" =>
          /\ db.events[i].gen = db.events[i].claimGen
          /\ db.events[i].session = db.events[i].holder
OnlyCurrentAuthorizedWrites ==
    \A i \in 1..Len(db.events) : db.events[i].permitted
IdempotentRequestsAuthorOnce ==
    \A i, j \in 1..Len(db.events) :
        (db.events[i].key # None /\ db.events[j].key # None /\
         db.events[i].principal = db.events[j].principal /\
         db.events[i].key = db.events[j].key) => i = j
IdempotencyMatchesEvent ==
    \A p \in Principals, k \in Keys :
       LET r == db.idem[p][k]
       IN r.event # 0 =>
          /\ r.event <= Len(db.events)
          /\ db.events[r.event].principal = p
          /\ db.events[r.event].key = k
          /\ db.events[r.event].gen = r.gen
UncertainCommitFencesSigning == ~signatureWhileFailed
HealthyMemoryMatchesStore == ~failed => mem = db

(* These intentionally fail when a requested witness is reachable. A run must
   name this exact invariant AND preserve a matching trace to count as evidence. *)
WitnessABADelayedReject ==
    ~(phase = 6 /\ lastOutcome = "Superseded" /\ db.holder = A /\
      db.generation = 3 /\ lastRequest.session = A /\ lastRequest.gen = 1 /\
      Len(db.events) = 1)
WitnessRestartReplay ==
    ~(phase = 9 /\ reopened /\ lastOutcome = "Replayed" /\
      db.holder = A /\ db.generation = 3 /\ Len(db.events) = 2)
WitnessFailureBeforeRecovery ==
    ~(phase = 9 /\ reopened /\ ~failed /\ lastOutcome = "Recorded" /\
      db.holder = A /\ db.generation = 1 /\ Len(db.events) = 2)
WitnessFailureAfterRecovery ==
    ~(phase = 9 /\ reopened /\ ~failed /\ lastOutcome = "Replayed" /\
      db.holder = A /\ db.generation = 1 /\ Len(db.events) = 2)
WitnessCancellationFence ==
    ~(phase = 5 /\ db.cancelled /\ lastOutcome = "Conflict" /\ Len(db.events) = 1)
WitnessPrincipalBoundary ==
    ~(phase = 3 /\ db.bindings[A] = Other /\ lastOutcome = "Denied" /\
      Len(db.events) = 0)
WitnessDepartureFence ==
    ~(Scenario = "departure" /\ phase = 8 /\ reopened /\ db.departed /\
      lastOutcome = "Denied" /\ Len(db.events) = 1)
WitnessDepartureReplay ==
    ~(Scenario = "departure-replay" /\ phase = 7 /\ db.departed /\
      lastOutcome = "Replayed" /\ Len(db.events) = 2)
=============================================================================
