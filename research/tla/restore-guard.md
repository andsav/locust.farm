# Restore guard: proposed G1/G2 model and evidence

Date: 2026-10-06. [RestoreGuard.tla](RestoreGuard.tla) models the proposed
[restore guard](../../docs/host-safety-and-ending-plan.md), before implementation.
It is independent of Organization.tla: local permission to sign is not validity
of a received record. The [companion scope](../../docs/host-safety-and-ending-plan-details.md)
and [owner decisions](../../docs/master-plan.md) remain unchanged.

The [retained results and full counterexample states](restore-guard-results.json)
identify the executed module/configuration hashes, pinned TLC/JDK, counts and
expected outcomes. These are finite model checks, not Rust conformance, filesystem
identity measurements or a proof for arbitrary log lengths.

## State and actions

There are three endpoints, `local`, `peer1`, `peer2`, and two keys, `gov` and
`agent`. Normally both keys belong to `local`; the member and rejoin cases put
the governance key on `peer1`. Other members are represented by endpoint
membership, without additional signing keys. Each endpoint stores record IDs.
The immutable `hist` is a verification observer of all signatures, including
signatures later lost everywhere; it is not available to the guard. Records
carry a key, position, governance membership view and agent-admission bit.
Distinct signatures at one position represent different signed bodies.

`Sign`, `Admit` and `Remove` update the owning store and its marks atomically.
There is at most one new admission and one removal, in either order; the initial
membership is a configuration input. `Copy` separately remembers the local
store and marks. `RestoreStore` retains valid installation marks;
`RestoreAll` restores both snapshots and invalidates their file identity.
`LoseMarks` followed by `Start` on the unchanged database reconstructs marks
without introducing a hold. A restored database has a different abstract file
identity. `Start` also raises marks below a stored tip. `Restart` is exercised
by the ordinary-start case. Only one copy/restore is explored in a behavior.

`Sync(a,b)` sends a complete record stage. Agent records arrive before governance:
when their admission is absent at the receiver, that stage drops them, even if
it then receives their admission. `Hear` records the peer only if the stage
added nothing. This permits signatures between exchange stages and settling;
it does not assume that one exchange reconstructs a log. A removed peer whose
store contains its removal answers with no records, as in the plan's named
counterexample. Unknown peers can call back and return governance records even
when absent from the restored copy's membership. Delivery of a removal to the
removed peer is permitted; that does not authorize subsequent agent traffic to it.

`Settle` implements G1's release table, including the never-shared exception,
agent give-up, the host's agent inheriting the governance hold, member recovery
and newly admitted agents. `Continue` clears holds and lowers marks, and has one
explicit fork witness; it is disabled in all claimed safety/liveness cases.

The fast general store case bounds the history at three records including the
founding record; the general all-data case uses four. Extended cases use five
and start with one peer, so admission, removal and a later conflicting signature
all fit. Directed scenarios use three to five records and the same actions,
with no separate transition implementation. Bounds limit experiments only.

## The six requested properties

All case IDs below have the prefix `restore-` in [cases.json](cases.json).

| Plan property | Cases and operators | Result and qualification |
| --- | --- | --- |
| 1. RestoreStore never reuses a signed position; agent exception only after removal | `p1-store`, `p1-store-extended`: `StoreNoFork`; `finding-private-reuse`, `finding-unseen-agent-reuse`: `StoreNoReuse` | The literal position-reuse claim fails for both keys, even without removal. The G1 give-up rules can forget records lost everywhere. The narrower absence of a fork among surviving stores passes within the stated removal exemption. See the findings note. |
| 2. Host RestoreAll cannot fork without Continue, despite changed membership | `p2-all`, `p2-all-extended`: `AllNoFork` | Holds at the checked bounds, with admission/removal in either order. Both local keys remain held. |
| 3. An ordinary healthy Start sets no hold | `p3-ordinary`: `OrdinaryStart` | Holds, including lost marks on the unchanged file. “Ordinary” excludes a mark ahead of the store, an existing unheard/admitted hold, and a replaced file; those are G1's explicit exceptions. |
| 4. Holds eventually end after RestoreStore with recoverable marks; host RestoreAll holds never end unaided | `p4-recovery`, `p4-alone`, `p4-agent-give-up`: `HoldsEnd`; `p4-willing-general`: `WillingHoldsEnd`; `p4-all-held` and `p2-all`: `AllStayHeld` | The positive recovery cases pass under the fairness assumptions below. The literal “a peer holds the marked record” condition fails when that peer has been removed and answers empty (`finding-removed-keeper`). Requiring a holder willing to send gives the checked conditional property. RestoreAll persistence passes without fairness or Continue. |
| 5. A host's agent never signs while governance is held | `p5-agent-fenced` and `p1-store`: `AgentFenced`; `agent-signs-held` removes the gate | Holds with the rule; the mutation reproduces a post on a view older than an admission. |
| 6. Every named counterexample is retained | The eight cases in the next table | All six rule mutations violate the intended property; the two planned residuals violate NoFork with every rule enabled. |

The [findings note](../restore-guard-model-2026-10-06.md) distinguishes the plan's
literal claims from the narrower properties that pass. Passing the runner means
all declared outcomes matched, including deliberate and discovered violations;
it does not mean every sentence of the plan is true.

## Named counterexamples

Each of the first six has a `-guarded` twin with its one mutation disabled. The
twin passes the same invariant; the scheduled unsafe action is then blocked.
Deadlock checking is disabled because waiting while held is intentional.

| Case | Removed rule and failing trace |
| --- | --- |
| `lacking-peer` | Release governance after hearing any one peer: Q retains a later governance record, P answers empty, L releases and signs at the retained position. |
| `empty-shared` | Ignore the shared bit when the peer list is empty: copy before first admission, admit Q and deliver to Q, restore, release, sign at Q's retained admission position. |
| `hear-with-data` | Count an exchange that added records as heard: after rejoin the first exchange drops the agent's position-zero record before accepting its admission; premature hearing releases both member holds and it signs at zero. |
| `give-agent-held` | Drop the requirement that governance is free before agent give-up: after losing both logs, an empty response causes the agent mark to be lowered while governance stays behind. `NoGiveWhileHeld` fails, even though the separate signing gate still holds the agent. |
| `agent-signs-held` | Remove only the inherited governance signing hold: copy before admission, admit Q, restore, post with the agent on the old membership view. `AgentFenced` fails. |
| `all-listed` | Release host RestoreAll after hearing the copied list: copy with P, remove P, admit Q, deliver the newer governance to Q and the removal to P, restore both store and marks; P answers empty, L releases and forks. Q is absent from the copied list. |
| `residual-removed` | No mutation: an agent record reaches only P; restore store, remove P, deliver current governance to Q; Q answers without the agent record, the mark is given up, and the agent reuses P's retained position. |
| `residual-member` | No mutation, local daemon is a member: agent record reaches Q but not host P; RestoreAll, hear empty host response, release and reuse Q's retained position. |

`continue-override` additionally shows that the owner's command may cause a
retained governance fork. It is a reachability demonstration, not a safety claim.

`restore-leave` (E2) models a restored host whose only other computer belongs to a
member that left. While held, the restored host signs no removal; the exchange with
that leaver peer runs to its end, and after `continue` releases the host, the removal
the host signs matches the record it signed before restoration (`RemovalMatchesBefore`).

## Liveness and runner contract

TLC checks actual temporal `PROPERTY` formulas (`~>` and `<>`), not a finite
“recovered” witness presented as eventual recovery. `FairSpec` gives the finite
setup weak fairness; `Reach` eventually makes each peer reachable, including
after restoration resets reachability. Once reachable, a peer remains reachable.
Peer selection for `Sync(peer,local)` is strongly fair because another pending
exchange can repeatedly interrupt its enabledness. `Hear` and `Settle` are
weakly fair. Histories are bounded, hence eventually quiet. No fairness is
assumed for signing or Continue. `FairGeneralSpec` applies the same recovery
fairness plus eventual Start to the unrestricted finite action system.

`p4-setup` checks that the directed recovery antecedent is eventually reached.
`p4-fairness-removed` removes Settle fairness: a never-shared host remains held
while empty peer exchanges repeat forever. It is an expected liveness failure
showing why scheduler progress is an assumption. The removed-holder finding
fails even with full recovery fairness. Neither result uses a clock.

[The runner](../../scripts/check_tla.py) validates `PROPERTY`/`PROPERTIES`
against `temporal_properties` in each registry entry. It permits one temporal
property per case because the pinned TLC prints no property name on temporal
failure. An expected temporal failure needs exit 13, normal completion, readable
states, a valid stuttering/back-edge target, exact registered `trace_loop`, and
matching state predicates. A temporal pass must complete temporal checking as
well as the state graph. Timeouts, parser failures, wrong properties and
incomplete traces fail. The fixtures cover a fair eventuality, an infinite
stutter and a genuine two-state cycle; unit tests cover malformed output.

```sh
/opt/homebrew/bin/python3 scripts/check_tla.py --bootstrap --suite fixtures
/opt/homebrew/bin/python3 scripts/check_tla.py --suite restore
/opt/homebrew/bin/python3 scripts/check_tla.py --suite fast
/opt/homebrew/bin/python3 scripts/check_tla.py --suite extended
/opt/homebrew/bin/python3 -m unittest discover -s scripts/tests
/opt/homebrew/bin/python3 scripts/check_docs.py
```

## Boundary

No Rust or Organization model is changed or qualified here. Excluded: byte
encoding, cryptography, torn writes and power loss, physical file identities,
in-place database overwrite combined with lost marks, simultaneous clones,
arbitrary transport framing and reconnect failures, gaps within author chains,
content keys, invitations, UI wording, public-service attestation, multiple goals,
unbounded traffic, and a second restore during recovery. Atomic mark durability,
authenticated records and the first table's file-identity detection are assumptions.
The in-place overwrite with kept marks uses the same `Behind` predicate, but is
not a separately explored physical action. No claim about actual APFS/ext4,
callbacks or daemon scheduling follows from a passing model.
