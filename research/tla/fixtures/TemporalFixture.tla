-------------------------- MODULE TemporalFixture --------------------------
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == x = 0 /\ x' = 1
Tick == x' = 1 - x
Spec == Init /\ [][Step]_x
FairSpec == Spec /\ WF_x(Step)
CycleSpec == Init /\ [][Tick]_x /\ WF_x(Tick)
TypeOK == x \in 0..1
EventuallyOne == <>(x = 1)
EventuallyTwo == <>(x = 2)
=============================================================================
