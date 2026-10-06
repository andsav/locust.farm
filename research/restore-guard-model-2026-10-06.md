# Restore guard model findings, 6 October 2026

Status: findings from the proposed [G1/G2 model](tla/restore-guard.md), before
implementation. The [plan](../docs/host-safety-and-ending-plan.md) and its
[companion](../docs/host-safety-and-ending-plan-details.md) are unchanged.
The model follows G1's start and release tables; it does not silently strengthen
them to make the “Models written first” paragraph pass. Full checked traces,
configurations and tool identities are retained in the
[verification record](tla/restore-guard-results.json).

## F1: property 1 is stronger than the release rules

The paragraph says that after RestoreStore, without Continue, governance never
signs at a used position, and the agent never does unless a Remove happened
after the copy. Both literal claims have counterexamples with every G1 rule
present and no removal.

Governance trace (`restore-finding-private-reuse`):

1. A goal has never had a member on another computer. Copy the founding store.
2. Sign governance record G1 at position 1. Its synced mark says never shared.
3. Restore the store, keep the mark, and Start. G1 is missing.
4. Settle applies G1's second release-table row: the mark says never shared and
   the copy shows no other computer, so it lowers the governance mark.
5. Sign a different governance record G2 at position 1, without Continue.

Agent trace (`restore-finding-unseen-agent-reuse`):

1. Copy a goal with peer P. Sign agent record A0 at position 0; send it nowhere.
2. Restore the store with marks kept and Start. Governance is current; A0 is missing.
3. Hear an empty exchange from P. Settle applies the agent give-up row and lowers
   the mark, because all current peers have answered and governance is not held.
4. Sign a different agent record A1 at position 0. No member was removed.

These are **position reuse, not forks among surviving copies**: the discarded
record is lost everywhere. G1 already describes these give-up rules and their
rationale, but the formal property paragraph does not exempt them. `StoreNoReuse`
fails; `StoreNoFork`, which compares the records surviving in endpoint stores,
passes at the checked bounds with the plan's agent-removal exemption. This is a
specification inconsistency, not evidence that these two deliberate release
rules should be removed. The claim must distinguish a record ever signed from
a conflicting record still held somewhere, or explicitly exempt global loss.

## F2: a reachable honest holder need not send its record

Property 4 says that with eventually reachable honest peers, the governance hold
ends when a peer holds the mark (or the goal was never shared). The plan also
expressly permits a removed peer to answer with nothing, in its RestoreAll
counterexample. Those two statements do not imply the proposed eventuality.

Trace (`restore-finding-removed-keeper`, no mutation):

1. Copy the host's store while P is the other member.
2. Remove P, signing governance G1, and deliver that removal to P. P is now the
   only other holder of G1. Its current view says it has been removed.
3. Restore the host's store, keeping the mark for G1, and Start. The copy still
   lists P and the shared bit stays true.
4. P becomes reachable and honestly follows the removed-peer behavior: its
   exchanges finish but send no records. The second peer also has nothing newer.
5. Hear and Settle run fairly. Empty exchanges continue forever. The host's
   governance and agent holds remain; no Continue occurs.

The retained trace is an infinite cycle, not merely a finite prefix that stopped
before recovery. Strongly fair service of each peer and weakly fair hearing and
settling still cannot fetch G1. G1's rule “only the marked record ends this
shared governance hold” is respected. What fails is the unqualified liveness
claim that possession plus reachability and honesty suffice. G1 already says a
hold lasts if no answering computer supplies the mark; it does not list this
specific honest, reachable, removed sole-holder trace.

The checked narrower formula `WillingHoldsEnd` requires a holder whose own
membership still permits it to send, or a never-shared goal. It passes in the
bounded general action system. This is a proposed qualification of the theorem,
not an implemented change or a new permission to release governance early.

## Other results and limits

All six named rule-removal counterexamples reproduce, with guarded twins that
preserve their invariant. Both intentionally open residual cases reproduce with
all rules enabled. Host RestoreAll isolation, ordinary healthy Start and the
host-agent gate pass. Positive temporal cases cover recovering both records,
giving up an agent record after governance recovers, and never-shared release.
A separate fairness mutation shows that scheduling Settle is necessary.

No additional fork among surviving copies was found inside the checked safety
bounds and stated exemptions. Histories contain at most five signatures including
the founding record, one admission and one removal; this is not an unbounded
proof. Full scope, fairness assumptions and exclusions are in the
[model guide](tla/restore-guard.md). File identity and real transport behavior
remain separate implementation/measurement obligations. No cargo command ran.
