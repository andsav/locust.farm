------------------------------ MODULE GoalLog ------------------------------
EXTENDS Integers, Sequences, FiniteSets, TLC

\* Stage 1 CURRENT-BEHAVIOR model, atomic storage, one goal. An event is an
\* immutable authenticated record; 0 is the absent ID and coordinator key.
\* The cfg selects a finite signed transcript, then explores all delivery
\* orders, gaps and duplicates. Scripted finding cases preserve the review's
\* causal prefix before delivering its fault. These are finite scenario
\* checks, not exhaustive generation of arbitrary event bodies or signatures.
\* Member keys sort numerically. Payload/key epochs, document/note contents,
\* endpoints, screening, transport and disk writes are outside this model.
\* Initial tasks have max_attempts=2; unbounded budgets, u32 overflow,
\* decline/failure/progress/rejection and cancellation outcome variants are
\* outside these initial transcript families. Notes are a no-op contribution.
\* Task dependencies/deadlines are metadata
\* in Rust and deliberately add no preconditions here.
\*
\* Mappings: history.rs usable/fork; chain.rs build/tenures/past_removal;
\* fold.rs scan/reference/fold; append.rs append/extend; transition.rs
\* assign/cancel/decidable/accept/record; state.rs task/assignment/results.
\* FullReplay independently constructs author-prefix and canonical order.
\* Increment uses structural append guards and directly updates the existing
\* projection. It falls back to FullReplay for non-append arrivals, as Rust
\* does; it conservatively refolds some arrivals Rust can append. The shared
\* transition algebra is intentional: equality checks incremental placement
\* and reference invalidation, not independent correctness of that algebra.

CONSTANTS Scenario, Scripted
VARIABLES held, incremental, everHeads, witnesses, lastDelivery,
  hasAcceptance, hasCancellation, hasRemoval, hasFork,
  acceptedHeadCount, acceptedHeadsLost, displayedAcceptedMismatch, hasAppendContribution
vars == <<held, incremental, everHeads, witnesses, lastDelivery,
  hasAcceptance, hasCancellation, hasRemoval, hasFork,
  acceptedHeadCount, acceptedHeadsLost, displayedAcceptedMismatch, hasAppendContribution>>

E(id, author, seq, prev, anchor, kind, task, assignment, ref, member,
  attempt, budget, base, head, cutoff) ==
  [id |-> id, author |-> author, seq |-> seq, prev |-> prev,
   anchor |-> anchor, kind |-> kind, task |-> task,
   assignment |-> assignment, ref |-> ref, member |-> member,
   attempt |-> attempt, budget |-> budget, base |-> base,
   head |-> head, cutoff |-> cutoff]

\* Baseline/IR-12 IDs map directly to Rust's characterization fixture:
\* 1 genesis, 2 own admission, 3 worker admission, 4 proposal, 5 assignment,
\* 6 take, 7 result, 8 accept, 9 another result after acceptance.
Base == <<
  E(1,0,0,0,0,"genesis",0,0,0,0,0,0,0,0,0),
  E(2,0,1,1,1,"admit",0,0,0,0,0,0,0,0,0),
  E(3,0,2,2,2,"admit",0,0,0,1,0,0,0,0,0),
  E(4,0,3,3,3,"propose",0,0,0,0,0,2,0,0,0),
  E(5,0,4,4,3,"assign",4,0,4,1,1,0,0,0,0),
  E(6,1,0,0,5,"take",0,5,0,0,0,0,0,0,0),
  E(7,1,1,6,5,"submit",0,5,0,0,0,0,0,0,0),
  E(8,0,5,5,5,"accept",0,0,7,0,0,0,0,8,0),
  E(9,1,2,7,8,"submit",0,5,0,0,0,0,0,0,0)>>

\* Cancellation of attempt 1, its later report retained as evidence, then
\* attempt 2, take, submit and accept. No assignment may exceed budget 2.
Cancel == SubSeq(Base,1,6) \o <<
  E(7,0,5,5,5,"cancel",0,5,0,0,0,0,0,0,0),
  E(8,1,1,6,7,"ack",0,5,7,0,0,0,0,0,0),
  E(9,0,6,7,7,"assign",4,0,4,1,2,0,0,0,0),
  E(10,1,2,8,9,"take",0,9,0,0,0,0,0,0,0),
  E(11,1,3,10,9,"submit",0,9,0,0,0,0,0,0,0),
  E(12,0,7,9,9,"assign",4,0,4,1,3,0,0,0,0),
  E(13,0,8,12,12,"accept",0,0,11,0,0,0,0,13,0)>>

\* Removing a worker back-fences its old contribution (cutoff absent), then
\* readmission allows a new tenure; old assignment stays revoked. Task state
\* is NOT assumed to change on removal, matching transition.rs.
Removal == SubSeq(Base,1,7) \o <<
  E(8,0,5,5,5,"remove",0,0,0,1,0,0,0,0,0),
  E(9,0,6,8,8,"admit",0,0,0,1,0,0,0,0,0),
  E(10,0,7,9,9,"assign",4,0,4,1,2,0,0,0,0),
  E(11,1,2,7,10,"take",0,10,0,0,0,0,0,0,0),
  E(12,1,3,11,10,"submit",0,10,0,0,0,0,0,0,0),
  E(13,0,8,10,10,"accept",0,0,12,0,0,0,0,13,0)>>

\* Same valid prefix plus a coordinator fork, or an invalid decision anchor.
CoordinatorFork == SubSeq(Base,1,8) \o <<
  E(9,0,2,2,2,"admit",0,0,0,2,0,0,0,0,0)>>
CanonicalOrder == SubSeq(Base,1,5) \o <<
  E(6,1,0,0,3,"propose",0,0,0,0,0,2,0,0,0),
  E(7,0,5,5,5,"note",0,0,0,0,0,0,0,0,0),
  E(8,1,1,6,5,"take",0,5,0,0,0,0,0,0,0),
  E(9,1,2,8,5,"submit",0,5,0,0,0,0,0,0,0),
  E(10,0,6,7,5,"accept",0,0,9,0,0,0,0,10,0)>>
CancelAcceptance == SubSeq(Base,1,7) \o <<
  E(8,0,5,5,5,"cancel",0,5,0,0,0,0,0,0,0),
  E(9,1,2,7,8,"submit",0,5,0,0,0,0,0,0,0),
  E(10,0,6,8,8,"accept",0,0,9,0,0,0,0,10,0)>>
MemberFork == SubSeq(Base,1,8) \o <<
  E(9,1,0,0,5,"note",0,0,0,0,0,0,0,0,0)>>
BrokenAnchor == SubSeq(Base,1,7) \o <<
  E(8,0,5,5,2,"accept",0,0,7,0,0,0,0,8,0)>>
\* A noncoordinator decision grants nothing. A usable later contribution
\* with a regressed anchor is excluded; an unknown anchor gates successors.
Authority == SubSeq(Base,1,5) \o <<
  E(6,1,0,0,5,"assign",4,0,4,1,2,0,0,0,0),
  E(7,1,1,6,5,"note",0,0,0,0,0,0,0,0,0),
  E(8,1,2,7,3,"note",0,0,0,0,0,0,0,0,0),
  E(9,1,3,8,99,"note",0,0,0,0,0,0,0,0,0),
  E(10,1,4,9,5,"note",0,0,0,0,0,0,0,0,0)>>

\* IR-5 full downstream example: worker 1 executes three tasks; worker 2
\* only proposes task 10. Heads 9,14,19 were accepted; event 20 forks that
\* proposal at author position 0, leaving only head 9. Event 21 removes the
\* proposer and cannot repair the excluded proposal/history.
IR5 == <<
  E(1,0,0,0,0,"genesis",0,0,0,0,0,0,0,0,0),
  E(2,0,1,1,1,"admit",0,0,0,0,0,0,0,0,0),
  E(3,0,2,2,2,"admit",0,0,0,1,0,0,0,0,0),
  E(4,0,3,3,3,"admit",0,0,0,2,0,0,0,0,0),
  E(5,0,4,4,4,"propose",0,0,0,0,0,2,0,0,0),
  E(6,0,5,5,4,"assign",5,0,5,1,1,0,0,0,0),
  E(7,1,0,0,6,"take",0,6,0,0,0,0,0,0,0),
  E(8,1,1,7,6,"submit",0,6,0,0,0,0,0,0,0),
  E(9,0,6,6,6,"accept",0,0,8,0,0,0,0,9,0),
  E(10,2,0,0,9,"propose",0,0,0,0,0,2,0,0,0),
  E(11,0,7,9,9,"assign",10,0,10,1,1,0,0,0,0),
  E(12,1,2,8,11,"take",0,11,0,0,0,0,0,0,0),
  E(13,1,3,12,11,"submit",0,11,0,0,0,0,9,0,0),
  E(14,0,8,11,11,"accept",0,0,13,0,0,0,0,14,0),
  E(15,0,9,14,14,"propose",0,0,0,0,0,2,0,0,0),
  E(16,0,10,15,14,"assign",15,0,15,1,1,0,0,0,0),
  E(17,1,4,13,16,"take",0,16,0,0,0,0,0,0,0),
  E(18,1,5,17,16,"submit",0,16,0,0,0,0,14,0,0),
  E(19,0,11,16,16,"accept",0,0,18,0,0,0,0,19,0),
  E(20,2,0,0,9,"note",0,0,0,0,0,0,0,0,0),
  E(21,0,12,19,19,"remove",0,0,0,2,0,0,0,0,0)>>

Transcript == CASE Scenario = "safety" -> SubSeq(Base,1,8)
  [] Scenario = "ir12" -> Base
  [] Scenario = "cancel" -> Cancel
  [] Scenario = "cancel-accept" -> CancelAcceptance
  [] Scenario = "removal" -> Removal
  [] Scenario = "coordinator-fork" -> CoordinatorFork
  [] Scenario = "member-fork" -> MemberFork
  [] Scenario = "broken-anchor" -> BrokenAnchor
  [] Scenario = "authority" -> Authority
  [] Scenario = "canonical-order" -> CanonicalOrder
  [] Scenario = "ir5" -> IR5
Ids == 1..Len(Transcript)
Event(i) == Transcript[i]
Authors == {Event(i).author : i \in Ids}
Principals == Authors \cup {Event(i).member : i \in Ids}
Tasks == {i \in Ids : Event(i).kind = "propose"}
Assignments == {i \in Ids : Event(i).kind = "assign"}
Results == {i \in Ids : Event(i).kind = "submit"}
Decision(e) == e.kind \in {"genesis","admit","remove","assign","cancel","accept"}
MaxSeq == Len(Transcript)
Minimum(S) == CHOOSE x \in S : \A y \in S : x <= y
Maximum(S) == CHOOSE x \in S : \A y \in S : x >= y
SetOf(s) == {s[j] : j \in 1..Len(s)}
LastOrZero(s) == IF Len(s) = 0 THEN 0 ELSE s[Len(s)]
Positions(h,a,q) == {i \in h : Event(i).author = a /\ Event(i).seq = q}
ForkAt(h,a) == Minimum({q \in 0..MaxSeq : Cardinality(Positions(h,a,q)) > 1}
                       \cup {MaxSeq+1})

RECURSIVE Prefix(_,_,_,_)
Prefix(h,a,q,prev) ==
  LET here == Positions(h,a,q) IN
  IF Cardinality(here) # 1 THEN <<>>
  ELSE LET i == CHOOSE x \in here : TRUE IN
       IF Event(i).prev # prev THEN <<>>
       ELSE <<i>> \o Prefix(h,a,q+1,i)
Usable(h,a) == Prefix(h,a,0,0)

RECURSIVE BuildChain(_,_,_)
BuildChain(log,at,links) ==
  IF at > Len(log) THEN [links |-> links, halt |-> "none", haltSeq |-> MaxSeq+1]
  ELSE LET i == log[at] IN
       IF ~Decision(Event(i)) THEN BuildChain(log,at+1,links)
       ELSE IF Event(i).anchor # LastOrZero(links)
       THEN [links |-> links, halt |-> "broken-anchor", haltSeq |-> Event(i).seq]
       ELSE BuildChain(log,at+1,Append(links,i))
Chain(h) ==
  LET c == BuildChain(Usable(h,0),1,<<>>) IN
  IF 1 \notin h THEN [links |-> <<>>, halt |-> "none", haltSeq |-> MaxSeq+1]
  ELSE
  IF c.halt # "none" THEN c
  ELSE IF ForkAt(h,0) <= MaxSeq
       THEN [c EXCEPT !.halt = "fork", !.haltSeq = ForkAt(h,0)] ELSE c
ChainPos(c,id) == IF id \in SetOf(c.links)
                 THEN CHOOSE j \in 1..Len(c.links) : c.links[j] = id ELSE 0

RECURSIVE MemberAt(_,_,_)
MemberAt(c,a,p) ==
  IF p = 0 THEN FALSE
  ELSE LET e == Event(c.links[p]) IN
       IF e.member = a /\ e.kind = "admit" THEN TRUE
       ELSE IF e.member = a /\ e.kind = "remove" THEN FALSE
       ELSE MemberAt(c,a,p-1)
RemovalApplied(c,p) == LET e == Event(c.links[p]) IN
  e.kind = "remove" /\ MemberAt(c,e.member,p-1)
\* Future removals cut off earlier anchored work until the next readmission.
PastRemoval(c,e,p) == \E r \in 1..Len(c.links) :
  /\ RemovalApplied(c,r) /\ Event(c.links[r]).member = e.author
  /\ p < Minimum({j \in (r+1)..Len(c.links) :
       Event(c.links[j]).kind = "admit" /\ Event(c.links[j]).member = e.author}
       \cup {MaxSeq+1})
  /\ LET cut == Event(c.links[r]).cutoff IN
       cut = 0 \/ e.seq > Event(cut).seq \/
         (e.seq = Event(cut).seq /\ e.id # cut)

Earlier(h,e) == {i \in SetOf(Usable(h,e.author)) : Event(i).seq < e.seq}
UnknownGate(h,c,e) == \E i \in Earlier(h,e) :
  ~Decision(Event(i)) /\ Event(i).anchor # 0 /\ ChainPos(c,Event(i).anchor) = 0
ReachBefore(h,c,e) == Maximum({0} \cup
  {ChainPos(c,Event(i).anchor) : i \in {j \in Earlier(h,e) :
    ~(Decision(Event(j)) /\ Event(j).author # 0)}})
PreStanding(h,c,i) ==
  LET e == Event(i) IN
  IF i \notin h THEN "absent"
  ELSE IF 1 \notin h THEN
    IF e.seq >= ForkAt(h,e.author) THEN "excluded-fork"
    ELSE IF i \notin SetOf(Usable(h,e.author)) THEN "pending-predecessor"
    ELSE "pending-anchor"
  ELSE IF e.author = 0 /\ e.seq >= c.haltSeq THEN "excluded-after-halt"
  ELSE IF e.author # 0 /\ e.seq >= ForkAt(h,e.author) THEN "excluded-fork"
  ELSE IF i \notin SetOf(Usable(h,e.author)) THEN "pending-predecessor"
  ELSE IF Decision(e) THEN
       IF e.author # 0 THEN "excluded-coordinator" ELSE "pending-anchor"
  ELSE IF e.anchor = 0 \/ UnknownGate(h,c,e) THEN "pending-predecessor"
  ELSE IF ChainPos(c,e.anchor) = 0 THEN "pending-anchor"
  ELSE "pending-anchor"
ContributionStanding(h,c,i) ==
  LET e == Event(i) p == ChainPos(c,e.anchor) IN
  IF PastRemoval(c,e,p) THEN "excluded-removal"
  ELSE IF p < ReachBefore(h,c,e) THEN "excluded-regression"
  ELSE IF ~MemberAt(c,e.author,p) THEN "excluded-membership"
  ELSE "pending-anchor"

EmptyState == [members |-> {}, tasks |-> {}, assignments |-> {}, results |-> {},
  taskOrder |-> <<>>, resultOrder |-> <<>>,
  taskState |-> [t \in Tasks |-> "absent"], attempt |-> [t \in Tasks |-> 0],
  current |-> [t \in Tasks |-> 0], result |-> [t \in Tasks |-> 0],
  accepted |-> [t \in Tasks |-> 0], cancel |-> [a \in Assignments |-> 0],
  revoked |-> [a \in Assignments |-> FALSE],
  assignmentResult |-> [a \in Assignments |-> 0],
  verdict |-> [r \in Results |-> "none"], heads |-> <<>>, head |-> 0]

\* Each transition returns an unchanged state on a refused precondition.
Reply(s,v) == [state |-> s, standing |-> v]
Refuse(s) == Reply(s,"excluded-precondition")
Effective(s) == Reply(s,"effective")
AssignmentTask(a) == Event(a).task
Live(s,a) == s.cancel[a] = 0 /\ ~s.revoked[a]
Decidable(s,r) ==
  IF r \notin s.results THEN FALSE
  ELSE LET a == Event(r).assignment t == AssignmentTask(a) IN
       s.verdict[r] = "none" /\ s.current[t] = a /\ s.result[t] = r /\
       s.accepted[t] = 0 /\ Live(s,a)

Apply(s,i) ==
  LET e == Event(i) IN
  CASE e.kind = "genesis" -> Effective(s)
  [] e.kind = "admit" ->
     IF e.member \in s.members THEN Refuse(s)
     ELSE Effective([s EXCEPT !.members = @ \cup {e.member}])
  [] e.kind = "remove" ->
     IF e.member \notin s.members THEN Refuse(s)
     ELSE Effective([s EXCEPT !.members = @ \ {e.member},
       !.revoked = [a \in Assignments |-> s.revoked[a] \/
                         (a \in s.assignments /\ Event(a).member = e.member)]])
  [] e.kind = "propose" -> Effective([s EXCEPT !.tasks = @ \cup {i},
                         !.taskState[i] = "proposed", !.taskOrder = Append(@,i)])
  [] e.kind = "assign" ->
     IF e.task \notin s.tasks THEN Refuse(s)
     ELSE IF e.member \notin s.members \/ s.accepted[e.task] # 0 \/
             e.attempt # s.attempt[e.task]+1 \/ e.attempt > Event(e.task).budget
          THEN Refuse(s)
     ELSE Effective([s EXCEPT !.assignments = @ \cup {i},
       !.current[e.task] = i, !.attempt[e.task] = e.attempt,
       !.result[e.task] = 0, !.taskState[e.task] = "assigned"])
  [] e.kind = "cancel" ->
     IF e.assignment \notin s.assignments THEN Refuse(s)
     ELSE LET a == e.assignment t == AssignmentTask(a) IN
       IF s.cancel[a] # 0 \/ (s.current[t] = a /\ s.accepted[t] # 0)
       THEN Refuse(s)
       ELSE Effective([s EXCEPT !.cancel[a] = i,
         !.taskState[t] = IF s.current[t] = a THEN "cancel-requested" ELSE @])
  [] e.kind = "accept" ->
     IF ~Decidable(s,e.ref) THEN Refuse(s)
     ELSE LET r == e.ref a == Event(r).assignment t == AssignmentTask(a) IN
       IF e.head # 0 /\ Len(s.heads) > 0 /\ LastOrZero(s.heads) # Event(r).base
       THEN Refuse(s)
       ELSE Effective([s EXCEPT !.accepted[t] = r, !.taskState[t] = "accepted",
         !.verdict[r] = "accepted", !.heads = IF e.head = 0 THEN @ ELSE Append(@,e.head)])
  [] e.kind \in {"take","submit"} ->
     IF e.assignment \notin s.assignments THEN Refuse(s)
     ELSE LET a == e.assignment t == AssignmentTask(a) IN
       IF Event(a).member # e.author THEN Refuse(s)
       ELSE IF s.current[t] = a /\ Live(s,a) /\ s.accepted[t] = 0 /\
          ((e.kind = "take" /\ s.taskState[t] # "assigned") \/
           (e.kind = "submit" /\ s.taskState[t] # "taken")) THEN Refuse(s)
       ELSE Effective([s EXCEPT
         !.results = IF e.kind = "submit" THEN @ \cup {i} ELSE @,
         !.resultOrder = IF e.kind = "submit" THEN Append(@,i) ELSE @,
         !.assignmentResult[a] = IF e.kind = "submit" THEN i ELSE @,
         !.result[t] = IF s.current[t] = a /\ e.kind = "submit" THEN i ELSE @,
         !.taskState[t] = IF s.current[t] = a /\ Live(s,a) /\ s.accepted[t] = 0
                         THEN IF e.kind = "take" THEN "taken" ELSE "submitted" ELSE @])
  [] e.kind = "ack" ->
     IF e.assignment \notin s.assignments THEN Refuse(s)
     ELSE LET a == e.assignment t == AssignmentTask(a) IN
       IF s.cancel[a] # e.ref \/ Event(a).member # e.author THEN Refuse(s)
       ELSE Effective([s EXCEPT !.taskState[t] =
         IF s.current[t] = a THEN "cancelled" ELSE @])
  [] e.kind = "note" -> Effective(s)
  [] OTHER -> Refuse(s)

EmptyProjection(h,c) == [state |-> EmptyState,
  standings |-> [i \in Ids |-> PreStanding(h,c,i)],
  halt |-> c.halt, applied |-> 0, lastAuthor |-> -1, lastSeq |-> 0]
ReferenceStatus(h,c,p,i) ==
  LET e == Event(i) r == e.ref IN
  IF e.kind \notin {"assign","accept"} THEN "clear"
  ELSE IF r \notin h THEN "wait"
  ELSE IF p.standings[r] = "effective" THEN "clear"
  ELSE IF p.standings[r] \in {"pending-anchor","pending-predecessor","pending-reference"}
       THEN IF ChainPos(c,Event(r).anchor) > 0 /\
               ChainPos(c,Event(r).anchor) <= p.applied THEN "wait" ELSE "refuse"
  ELSE "refuse"
Decide(h,c,p,i) ==
  LET ref == ReferenceStatus(h,c,p,i)
      applied == IF ref = "clear" THEN Apply(p.state,i) ELSE Refuse(p.state) IN
  IF ref = "wait" THEN [p EXCEPT !.standings[i] = "pending-reference"]
  ELSE [p EXCEPT !.state = [applied.state EXCEPT !.head = i],
    !.standings[i] = applied.standing, !.applied = @+1,
    !.lastAuthor = -1, !.lastSeq = 0]
Contribute(h,c,p,i) ==
  LET standing == ContributionStanding(h,c,i)
      result == IF standing = "pending-anchor" THEN Apply(p.state,i)
                ELSE Reply(p.state,standing) IN
  [p EXCEPT !.state = result.state, !.standings[i] = result.standing,
    !.lastAuthor = IF standing = "pending-anchor" THEN Event(i).author ELSE @,
    !.lastSeq = IF standing = "pending-anchor" THEN Event(i).seq ELSE @]
Before(i,j) == Event(i).author < Event(j).author \/
  (Event(i).author = Event(j).author /\ Event(i).seq < Event(j).seq)
NextContribution(S) == CHOOSE i \in S : \A j \in S \ {i} : Before(i,j)
RECURSIVE ApplyContributions(_,_,_,_)
ApplyContributions(h,c,p,S) == IF S = {} THEN p ELSE
  LET i == NextContribution(S) IN ApplyContributions(h,c,Contribute(h,c,p,i),S \ {i})
Candidates(h,c,anchor) == {i \in h : ~Decision(Event(i)) /\
  i \in SetOf(Usable(h,Event(i).author)) /\ Event(i).anchor = anchor /\
  ~UnknownGate(h,c,Event(i)) /\
  Event(i).seq < IF Event(i).author = 0 THEN c.haltSeq ELSE ForkAt(h,Event(i).author)}
RECURSIVE Walk(_,_,_,_)
Walk(h,c,p,at) ==
  IF at > Len(c.links) THEN p
  ELSE LET i == c.links[at] q == Decide(h,c,p,i) IN
       IF q.applied = p.applied THEN q
       ELSE Walk(h,c,ApplyContributions(h,c,q,Candidates(h,c,i)),at+1)
FullReplay(h) == LET c == Chain(h) IN Walk(h,c,EmptyProjection(h,c),1)

\* Conservative append guards determined without evaluating FullReplay(h).
Appendable(h,p,i) ==
  LET e == Event(i) old == h \ {i} c == Chain(old) IN
  /\ p.halt = "none" /\ Len(c.links) > 0 /\ p.applied = Len(c.links)
  /\ ForkAt(h,e.author) > MaxSeq
  /\ i = LastOrZero(Usable(h,e.author))
  /\ Len(Usable(h,e.author)) = Len(Usable(old,e.author))+1
  /\ Cardinality({j \in old : Event(j).author = e.author}) = Len(Usable(old,e.author))
  /\ \A j \in old : p.standings[j] # "pending-reference" /\
          (Event(j).anchor # i /\ Event(j).ref # i)
  /\ IF Decision(e)
       THEN e.author = 0 /\ e.kind # "remove" /\ e.anchor = p.state.head
       ELSE e.anchor = p.state.head /\ ~UnknownGate(h,c,e) /\
         (p.lastAuthor < e.author \/ (p.lastAuthor = e.author /\ p.lastSeq < e.seq))
Increment(h,p,i) ==
  IF Appendable(h,p,i) THEN
    LET c == Chain(h)
        prepared == [p EXCEPT !.standings[i] = PreStanding(h,c,i)] IN
    IF Decision(Event(i)) THEN Decide(h,c,prepared,i)
    ELSE Contribute(h,c,prepared,i)
  ELSE FullReplay(h)

Witness(p) == [appendContribution |-> FALSE, acceptance |-> \E t \in p.state.tasks : p.state.accepted[t] # 0,
  cancellation |-> \E a \in p.state.assignments : p.state.cancel[a] # 0,
  removal |-> \E a \in p.state.assignments : p.state.revoked[a],
  forkDetection |-> p.halt = "fork" \/
    \E i \in Ids : p.standings[i] = "excluded-fork"]
UnionWitness(a,b) == [k \in DOMAIN a |-> a[k] \/ b[k]]
Mismatch(p) == \E t \in p.state.tasks : p.state.accepted[t] # 0 /\
  p.state.result[t] # p.state.accepted[t]
Observe == /\ hasAcceptance = witnesses.acceptance
  /\ hasCancellation = witnesses.cancellation /\ hasRemoval = witnesses.removal
  /\ hasFork = witnesses.forkDetection
  /\ acceptedHeadCount = Len(incremental.state.heads)
  /\ acceptedHeadsLost = ~(everHeads \subseteq SetOf(incremental.state.heads))
  /\ displayedAcceptedMismatch = Mismatch(incremental)
  /\ hasAppendContribution = witnesses.appendContribution
Init == /\ held = {} /\ incremental = FullReplay({}) /\ everHeads = {}
        /\ witnesses = Witness(incremental) /\ lastDelivery = 0 /\ Observe
Deliver(i) ==
  /\ i \in Ids \ held
  /\ ~Scripted \/ i = Cardinality(held)+1
  /\ held' = held \cup {i}
  /\ incremental' = Increment(held',incremental,i)
  /\ everHeads' = everHeads \cup SetOf(incremental'.state.heads)
  /\ witnesses' = UnionWitness(witnesses,[Witness(incremental') EXCEPT
       !.appendContribution = Appendable(held',incremental,i) /\ ~Decision(Event(i))])
  /\ lastDelivery' = i
  /\ hasAcceptance' = witnesses'.acceptance
  /\ hasCancellation' = witnesses'.cancellation
  /\ hasRemoval' = witnesses'.removal
  /\ hasFork' = witnesses'.forkDetection
  /\ acceptedHeadCount' = Len(incremental'.state.heads)
  /\ acceptedHeadsLost' = ~(everHeads' \subseteq SetOf(incremental'.state.heads))
  /\ displayedAcceptedMismatch' = Mismatch(incremental')
  /\ hasAppendContribution' = witnesses'.appendContribution
\* Duplicate delivery is explicitly an enabled no-op. Terminal quiescence
\* has a self-loop. Duplicate/no-op delivery means TLC deadlock checking
\* cannot establish protocol progress; this stage checks safety only.
Duplicate == /\ held # {} /\ UNCHANGED vars
Quiescent == /\ held = Ids /\ UNCHANGED vars
Next == (\E i \in Ids : Deliver(i)) \/ Duplicate \/ Quiescent
Spec == Init /\ [][Next]_vars

TypeOK == /\ held \subseteq Ids /\ everHeads \subseteq Ids
  /\ incremental.state.members \subseteq Principals
  /\ incremental.state.tasks \subseteq Tasks
  /\ SetOf(incremental.state.taskOrder) = incremental.state.tasks
  /\ SetOf(incremental.state.resultOrder) = incremental.state.results
  /\ incremental.state.assignments \subseteq Assignments
  /\ incremental.state.results \subseteq Results
  /\ incremental.state.current \in [Tasks -> Assignments \cup {0}]
  /\ incremental.state.attempt \in [Tasks -> 0..MaxSeq]
  /\ incremental.state.result \in [Tasks -> Results \cup {0}]
  /\ incremental.state.accepted \in [Tasks -> Results \cup {0}]
  /\ incremental.state.revoked \in [Assignments -> BOOLEAN]
  /\ incremental.halt \in {"none","fork","broken-anchor"}
  /\ witnesses \in [ {"appendContribution","acceptance","cancellation","removal","forkDetection"} -> BOOLEAN]
IncrementalEqualsReplay == incremental = FullReplay(held)
CoordinatorAuthority == \A i \in held : Decision(Event(i)) /\
  incremental.standings[i] = "effective" => Event(i).author = 0
AssignmentAttempts == \A t \in incremental.state.tasks :
  /\ incremental.state.attempt[t] <= Event(t).budget
  /\ LET a == incremental.state.current[t] IN
       a = 0 \/ (a \in incremental.state.assignments /\
         Event(a).task = t /\ Event(a).attempt = incremental.state.attempt[t])
AcceptedPreconditions == \A t \in incremental.state.tasks :
  LET r == incremental.state.accepted[t] IN
  r = 0 \/ (r \in incremental.state.results /\
    Event(r).assignment = incremental.state.current[t] /\
    incremental.state.verdict[r] = "accepted" /\
    incremental.state.cancel[Event(r).assignment] = 0)
\* Removal can revoke an already accepted assignment; acceptance is not
\* assumed to disappear solely due to revocation. This checks the fence on
\* every assignment that existed when its assignee's removal was applied.
RemovalFencesAssignments == \A a \in incremental.state.assignments :
  \A r \in SetOf(Chain(held).links) : Event(r).kind = "remove" /\
    incremental.standings[r] = "effective" /\ Event(r).member = Event(a).member /\
    ChainPos(Chain(held),a) < ChainPos(Chain(held),r) => incremental.state.revoked[a]
AcceptedHeadsPermanent == everHeads \subseteq SetOf(incremental.state.heads)
AcceptedResultDisplayed == \A t \in incremental.state.tasks :
  incremental.state.accepted[t] # 0 =>
    incremental.state.result[t] = incremental.state.accepted[t]
NoAppendContribution == ~witnesses.appendContribution
NoAcceptance == ~witnesses.acceptance
NoCancellation == ~witnesses.cancellation
NoRemoval == ~witnesses.removal
NoForkDetection == ~witnesses.forkDetection
=============================================================================
