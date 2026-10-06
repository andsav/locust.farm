# Workspace authority model

Status: executable bounded model of the proposed shared-tree contract, 2026-10-05.
The [implementation plan](../../docs/shared-file-tree-plan.md) and
[design review](../shared-file-tree-review-2026-10-05.md) define the product scope.
[Workspace.tla](Workspace.tla) checks signed-history authority independently of
file availability. This is neither a proof of the Rust implementation nor a
network qualification result.

Phase 4 boundary, 2026-10-06: the model's first tree change is a member's and
requires approval like later changes. It does not model the host agent's first
files counting when posted, or the only-member completion part. Those claims
rest on the Phase 4 Rust tests, including
`the_hosts_first_files_count_as_posted_whatever_the_trees_rule`,
`first_files_are_once_per_epoch_that_starts_empty` and
`first_files_and_the_only_member_part_never_disagree` in
[workspace tests](../../crates/locust-core/src/goal/workspace_tests.rs).
The workspace model is unchanged by this phase.

## Contract fixed by the model

The administrator event has an expected predecessor epoch, exact rules binding,
and one typed checkpoint choice:

```text
WorkspaceEpoch {
    expected_epoch: optional workspace-epoch event ID,
    rules: exact rules-binding event ID,
    checkpoint: Unseeded
              | Revision(workspace-selection event ID)
              | RetainBefore { epoch: ancestor workspace-epoch event ID }
}
```

The newest structurally valid epoch fences earlier work immediately, including
when its checkpoint or policy proof is pending. Governance does not validate work
proofs and can still admit/remove members or order a repair. A checkpoint validates
the exact selected predecessor chain, source proposals and candidate evidence.
It ignores competing workspace selection successors on that chosen chain; an
arbitrary non-selection signer fork still invalidates it. Exact proof pins never
authorize the pinned branches globally.

`Unseeded` is permitted only when the preceding epoch's structurally inherited
effective checkpoint is empty. It explicitly fences old seed decisions, including
ones delivered later; replay does not infer that a seed is absent. A nonempty
retained boundary therefore cannot disappear through an ordinary empty handoff.

`RetainBefore` explicitly restores the boundary inherited by its named ancestor
epoch. Its target must be in the verified predecessor epoch chain. The event
cancels that attempted transition and the intervening epochs, even if their
checkpoint evidence later arrives and is valid. This is administrator rollback
authority, including restoration of an earlier empty boundary. It is not a test of
local missingness and not an assertion that the canceled checkpoint was invalid.
The restored boundary must still validate before new selections can advance.

A rule forbidding emptiness after *any named checkpoint* strands an initial
missing/invalid checkpoint. An immediate-predecessor-only restoration also fails:
empty E0 → attempted bad checkpoint E1 → attempted bad checkpoint E2 → restore
before E2 still restores E1's bad checkpoint. Naming E1 explicitly restores E0's
empty boundary. The model includes both an initial invalid/missing attempt and
consecutive attempts above a retained seed, followed by ancestor restoration.

Proposal sources are typed same-goal proposal IDs forming a complete acyclic DAG.
Independent reviewers exclude the candidate author and every transitive source
author. Accepted parent ancestors do not enlarge that set. Evidence binds the
exact new candidate; reviews of its source or parent are insufficient.

## Bounds and observables

Each case selects one immutable finite transcript. There is one goal, with an
adversarial second goal in cross-goal cases; five non-administrator principals;
two policy definitions; at most fourteen signed events; and at most five workspace epochs. Event identities,
author slots and all transcript records are written directly in the model.
These bounds limit the verification experiment, not production history or swarm
size. TLC exhausts all event-delivery orders, with duplicate delivery represented
by stuttering. A missing policy can arrive independently.

The `view` record contains the current fenced epoch, checkpoint readiness,
authoritative head, historical valid selections, ordinary unpinned work and
governance prefix. Historical selections are distinct from the active epoch's
head. `contentState` reports `NoRevision`, `Missing`, `Invalid` or `Ready` and
does not alter authority. Content arrival may make a result usable; the content
witness can advance to a replacement without receiving the old manifest.

The registered safety properties cover:

- Deterministic replay over the held event set and content-independent authority.
- Pinned named integration authority, exact candidate evidence and typed complete
  acyclic source closure with transitive author exclusion.
- Membership-removal evidence fencing, current-epoch fencing and intrinsic
  checkpoint authority.
- Ordinary empty-boundary preservation, scope-local fork proofs and governance
  independence from missing or invalid workspace proofs.

The [case registry](cases.json) records 26 safety cases, 18 requested reachability
witnesses and ten deliberate mutations. Witnesses cover seed and handoff,
selection/reviewer forks, late forks/seeds, pending/invalid/repeated checkpoint
repair, preserved boundary refusal, historical-author eligibility, missing policy,
cyclic source refusal, administrator fork retraction and content readiness/repair.
Each mutation disables one guard and must violate its registered property with
the required final state predicate. Parser errors and unrelated failures do not
count as detected mutations.

Run the targeted cases using the existing runner's `--case` option:

```sh
/opt/homebrew/bin/python3.12 - <<'PY'
import json
import subprocess
from pathlib import Path
cases = json.loads(Path('research/tla/cases.json').read_text())['cases']
ids = [case['id'] for case in cases if case['id'].startswith('workspace-')]
raise SystemExit(subprocess.call([
    '/opt/homebrew/bin/python3.12', 'scripts/check_tla.py',
    *[arg for case in ids for arg in ('--case', case)]
]))
PY
```

Workspace cases also belong to `fast` and `extended`; their `workspace` registry
label currently requires the explicit selection above because the runner's CLI
suite choices remain unchanged. Bootstrap and tool/resource semantics are
described in the [runner guide](README.md).

## Proof boundary and implementation checks

The immutable input transcripts stand for authenticated decoded events; signatures,
wire canonicalization and administrator admission/authorization are assumptions.
Author slots approximate signer-fork positions. The model does not generate
arbitrary author logs, enforce full signer predecessor ancestry, or reproduce
governance-anchor monotonicity. Removal fencing is deliberately conservative:
removing a principal invalidates all of its modeled evidence. Rust tests must
cover exact retained admission cutoffs, readmission and anchor/tenure semantics.

Source checks model type, goal, complete closure, DAG shape and member revocation.
They do not reproduce every concrete proposal author-chain or source-parent
admissibility condition. Checkpoint ancestry checks model epoch lineage and exact
decision predecessors; they do not model arbitrary proposal bases excluded by an
earlier reconciliation. Those concrete proof and lineage cases need Rust tests.

There is no manifest decoding, key rotation, withdrawal-serving policy, filesystem
mutation, local grant validation, local idempotency store or crash journal here.
The content-invalid flag is a trusted validator observation; the model only proves
that this observation does not silently change selection authority. Checksum,
safe-path, exact-size, materialization and full-snapshot-repair behavior require
implementation tests. No fairness, network liveness, scalability or real-host
result follows from these bounded checks.

## Observed verification

The final frozen-model run under
`output/tla/runs/20261005T083929Z-171b98d5/` matched all 54 registered outcomes:

| Case kind | Expected outcomes | Distinct states examined | Generated states |
| --- | --- | --- | --- |
| Safety | 26/26 normal completions, all queues empty | 22,832 | 141,242 |
| Reachability | 18/18 exact `NeverWitness` violations | 12,635 | 76,283 |
| Guard mutations | 10/10 exact named violations | 3,617 | 17,800 |

Counts sum separate experiments, rather than counting unique states across
different models. The repeated-repair safety case alone exhausted 4,096 distinct
states (28,673 generated, depth 13); fork recovery exhausted 1,024 (6,145
generated, depth 11). The final model SHA-256 was
`79c14d913873f1969d9d1de8aac286ee0347ce23d4139e3f68723fd671fe992e`.
Pinned TLC/JDK identity, collision estimates, raw logs, frozen configs and exact
counterexample traces remain in that run's results.

The overall runner returned 1 because concurrent Rust implementation edits changed
its recorded source hashes during the run. No frozen model/config input changed;
every model result matched. This is bounded proposed-contract evidence, not a
passing stable-source runtime-conformance gate. The initial four safety cases
also completed under `output/tla/runs/20261005T083202Z-c6e8eaa6/`. A preceding
full run exposed two mutation fixtures blocked by additional guards; independent
reviewer and competing-seed fixtures isolated those guards before the final run.
The unchanged runner's 21 unit tests passed, and the model diff whitespace check
passed. Documentation index/link checks are deferred to the owner who stages the
new tracked documents.

A subsequent `--suite workspace` run at
`output/tla/runs/20261005T092709Z-85262876/` matched all 54 outcomes and returned
0 with `source_changed_during_run: false`. The frozen model and cases were
unchanged. This verifies a stable-source model run; it still does not establish
automatic conformance between the model and the Rust runtime.

The registry labels the modeled baseline as a proposed workspace
contract based on source commit `5e9d5927c681ac5b36ccfb28d91347001cfe0700`,
separately from the current running checkout. Runtime conformance is not claimed.
