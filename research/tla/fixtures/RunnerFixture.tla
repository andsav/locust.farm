--------------------------- MODULE RunnerFixture ---------------------------
EXTENDS Naturals
CONSTANT Limit
VARIABLE x
Init == x = 0
Next == IF x < 3 THEN x' = x + 1 ELSE UNCHANGED x
TypeOK == x \in 0..3
BelowLimit == x < Limit
=============================================================================
