----------------------------- MODULE FlowEffects -----------------------------
EXTENDS Naturals, Sequences, FiniteSets, TLC

(***************************************************************************
Daemon-driven configured-effect subset. Logical identity is a tuple of pinned
round, transition, trigger, target and action; witness choice and signed-event
identity are not its identity. Two signatures represent the same round-1 effect;
a third represents round 2. The model abstracts collision-free hashing.

Materialized signatures are the durable outbox source. Duplicate signatures
still project one task per logical id. Delivery, signed recipient acknowledgment
and locally permitted attempt start are separate transitions. Evidence retraction
stops outstanding delivery and future starts; immutable signatures remain held.
Atomic transactions and unforgeable configured signers/recipients are assumed.
Network liveness, processes, encryption and power-loss durability are excluded.
***************************************************************************)
CONSTANTS Scenario, Deduplicate, SeparateAcknowledgment
Signatures == {1,2,3}
Effects == {1,2}
Logical(sig) == IF sig \in {1,2} THEN 1 ELSE 2
Identity(effect) == [round |-> effect, transition |-> "open-stage", trigger |-> "completion",
                    target |-> "worker", action |-> "task"]
EmptyDB == [materialized |-> {}, tasks |-> <<>>, acknowledged |-> {}, started |-> {},
           permission |-> FALSE, evidence |-> {}, authorizedStarts |-> TRUE]
VARIABLES db, mem, failed, delivered, phase, lastOutcome, witnessReached, signingWhileFailed
vars == <<db,mem,failed,delivered,phase,lastOutcome,witnessReached,signingWhileFailed>>
Init == /\ db = EmptyDB /\ mem = EmptyDB /\ failed = FALSE /\ delivered = {}
 /\ phase = 0 /\ lastOutcome = "Initial" /\ witnessReached = FALSE /\ signingWhileFailed = FALSE
Observe(x) == IF Scenario = "safety" THEN "unobserved" ELSE x
Advance == phase' = IF Scenario = "safety" THEN phase ELSE phase + 1
HeldEffects(d) == {Logical(sig) : sig \in d.materialized}
Outbox(d) == (HeldEffects(d) \cap d.evidence) \ d.acknowledged
AddSignature(d,sig) == LET id == Logical(sig)
 IN [d EXCEPT !.materialized = @ \cup {sig},
      !.tasks = IF Deduplicate /\ id \in {d.tasks[i] : i \in 1..Len(d.tasks)}
                THEN @ ELSE Append(@,id)]
Commit(next,mode,authors) ==
 /\ mode \in {"success","before","after"}
 /\ db' = IF mode = "before" THEN db ELSE next
 /\ mem' = IF mode = "success" THEN next ELSE mem
 /\ failed' = (mode # "success")
 /\ signingWhileFailed' = (signingWhileFailed \/ (authors /\ failed))
 /\ UNCHANGED delivered /\ Advance
Evidence(id) == /\ ~failed /\ id \in Effects \ mem.evidence
 /\ Commit([mem EXCEPT !.evidence = @ \cup {id}],"success",FALSE)
 /\ lastOutcome' = Observe("Evidence")
Drive(sig,mode) == /\ ~failed /\ sig \in Signatures
 /\ Logical(sig) \in mem.evidence \ HeldEffects(mem)
 /\ Commit(AddSignature(mem,sig),mode,TRUE)
 /\ lastOutcome' = Observe(IF mode = "success" THEN "Materialized" ELSE "Internal")
ReceiveDuplicate(sig) == /\ ~failed /\ sig \in Signatures \ mem.materialized
 /\ Logical(sig) \in HeldEffects(mem) \cap mem.evidence
 /\ Commit(AddSignature(mem,sig),"success",FALSE)
 /\ lastOutcome' = Observe("DuplicateSignature")
Deliver(id) == /\ ~failed /\ id \in Outbox(mem) /\ delivered' = delivered \cup {id}
 /\ UNCHANGED <<db,mem,failed,signingWhileFailed>> /\ Advance
 /\ lastOutcome' = Observe("Delivered")
Acknowledge(id) == /\ ~failed /\ id \in HeldEffects(mem) \cap mem.evidence
 /\ id \notin mem.acknowledged
 /\ LET next == [mem EXCEPT !.acknowledged = @ \cup {id},
       !.started = IF SeparateAcknowledgment THEN @ ELSE @ \cup {id},
       !.authorizedStarts = @ /\ (SeparateAcknowledgment \/ mem.permission)]
    IN Commit(next,"success",TRUE)
 /\ lastOutcome' = Observe("Acknowledged")
Permission(value) == /\ ~failed /\ value \in BOOLEAN /\ mem.permission # value
 /\ Commit([mem EXCEPT !.permission = value],"success",FALSE)
 /\ lastOutcome' = Observe("PermissionChanged")
Start(id) == /\ ~failed /\ mem.permission
 /\ id \in HeldEffects(mem) \cap mem.evidence /\ id \notin mem.started
 /\ Commit([mem EXCEPT !.started = @ \cup {id}],"success",TRUE)
 /\ lastOutcome' = Observe("Started")
Retract(id) == /\ ~failed /\ id \in mem.evidence
 /\ Commit([mem EXCEPT !.evidence = @ \ {id}],"success",FALSE)
 /\ lastOutcome' = Observe("EvidenceRetracted")
Reopen == /\ mem' = db /\ failed' = FALSE
 /\ UNCHANGED <<db,delivered,signingWhileFailed>> /\ Advance
 /\ lastOutcome' = Observe("Reopened")
Quiesce == UNCHANGED <<db,mem,failed,delivered,phase,lastOutcome,signingWhileFailed>>
SafetyNext ==
 \/ \E id \in Effects : Evidence(id) \/ Deliver(id) \/ Acknowledge(id) \/ Start(id) \/ Retract(id)
 \/ \E sig \in Signatures, mode \in {"success","before","after"} : Drive(sig,mode)
 \/ \E sig \in Signatures : ReceiveDuplicate(sig)
 \/ \E value \in BOOLEAN : Permission(value)
 \/ Reopen \/ Quiesce
AckNext ==
 \/ /\ phase = 0 /\ Evidence(1)
 \/ /\ phase = 1 /\ Drive(1,"success")
 \/ /\ phase = 2 /\ Deliver(1)
 \/ /\ phase = 3 /\ Acknowledge(1)
 \/ /\ phase = 4 /\ Quiesce
DuplicateNext ==
 \/ /\ phase = 0 /\ Evidence(1)
 \/ /\ phase = 1 /\ Drive(1,"success")
 \/ /\ phase = 2 /\ ReceiveDuplicate(2)
 \/ /\ phase = 3 /\ Evidence(2)
 \/ /\ phase = 4 /\ Drive(3,"success")
 \/ /\ phase = 5 /\ Quiesce
RecoveryNext(mode) ==
 \/ /\ phase = 0 /\ Evidence(1)
 \/ /\ phase = 1 /\ Drive(1,mode)
 \/ /\ phase = 2 /\ Reopen
 \/ /\ phase = 3 /\ (IF mode = "before" THEN Drive(1,"success") ELSE Deliver(1))
 \/ /\ phase = 4 /\ Quiesce
RetractionNext ==
 \/ /\ phase = 0 /\ Evidence(1)
 \/ /\ phase = 1 /\ Drive(1,"success")
 \/ /\ phase = 2 /\ Deliver(1)
 \/ /\ phase = 3 /\ Retract(1)
 \/ /\ phase = 4 /\ Quiesce
ScheduledNext == CASE Scenario = "safety" -> SafetyNext
 [] Scenario = "ack" -> AckNext [] Scenario = "duplicate" -> DuplicateNext
 [] Scenario = "failure-before" -> RecoveryNext("before")
 [] Scenario = "failure-after" -> RecoveryNext("after")
 [] Scenario = "retraction" -> RetractionNext
Witness == CASE Scenario = "ack" -> phase = 4 /\ delivered = {1} /\
 mem.acknowledged = {1} /\ mem.started = {} /\ ~mem.permission
 [] Scenario = "duplicate" -> phase = 5 /\ db.materialized = {1,2,3} /\ Len(db.tasks) = 2
 [] Scenario \in {"failure-before","failure-after"} -> phase = 4 /\ ~failed /\
 db.materialized = {1} /\ Len(db.tasks) = 1 /\ mem = db
 [] Scenario = "retraction" -> phase = 4 /\ db.materialized = {1} /\ Outbox(mem) = {} /\ mem.started = {}
 [] OTHER -> FALSE
Next == /\ ScheduledNext /\ witnessReached' = Witness'
Spec == Init /\ [][Next]_vars
TypeOK == /\ db.materialized \subseteq Signatures /\ mem.materialized \subseteq Signatures
 /\ db.acknowledged \subseteq Effects /\ db.started \subseteq Effects
 /\ delivered \subseteq Effects /\ phase \in 0..5 /\ failed \in BOOLEAN
LogicalTaskDeduplication == Len(db.tasks) = Cardinality(HeldEffects(db))
OnlyPermittedStarts == db.authorizedStarts
OutboxHasDurableSource == Outbox(mem) \subseteq HeldEffects(db)
AcknowledgmentHasMaterialization == db.acknowledged \subseteq HeldEffects(db)
RoundsHaveDistinctIdentity == Identity(1) # Identity(2)
HealthyMemoryMatchesStore == ~failed => mem = db
UncertainCommitFencesSigning == ~signingWhileFailed
NeverWitness == ~witnessReached
=============================================================================
