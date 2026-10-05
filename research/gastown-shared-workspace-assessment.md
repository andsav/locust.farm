# Gas Town lessons for Locust shared workspaces

Status: source assessment, 2026-10-04; recommendations are proposed, not implemented
Locust behavior. Complements the [shared-tree implementation plan](../docs/shared-file-tree-plan.md).

Inspected [Gas Town](https://github.com/gastownhall/gastown) at commit
[`649b832b7672bc7a2dbef26f5983aba6198b819b`](https://github.com/gastownhall/gastown/commit/649b832b7672bc7a2dbef26f5983aba6198b819b),
whose recorded commit date is 2026-07-23. Locust baseline is
`e60c331b6e28bf8fda8c0d096eb5c9ad42735ff7`. This investigation read source,
test definitions and documentation; it did not install Gas Town, execute its
agents/scripts or run its tests. No throughput, reliability or scale benchmark
is claimed. All external code links below pin the inspected commit.

## Conclusion

Gas Town's workers edit isolated Git worktrees, and an integration process
assembles the common project state. Its most useful
lessons are about work surviving sessions, durable submission/landing evidence,
checkout recovery and dependent tasks starting from integrated work.

It is not an implementation of Locust's proposed decentralized shared-tree
authority. Use its operational workflow as prior art while retaining Locust's
signed events, encrypted artifacts, formation rules and local permissions.

## 1. Separate identity, session, checkout and accepted project state

Worker creation selects a base and creates a distinct branch/worktree; resumption
can attach existing work. See [worktree creation](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/polecat/manager.go#L733-L810).
The [lifecycle types](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/polecat/types.go#L8-L24)
distinguish persistent identity, temporary sessions and worktrees retained while
work or cleanup remains. This is not a single directory shared by every agent.

**Apply:** keep Locust's explicit checkout IDs and base revisions. A session restart
resumes its existing checkout/attempt rather than silently creating a new attempt
or resetting files to the current head. A new assignment can choose a newer
accepted revision. The accepted Locust plan uses ordinary directories and plain
file copies, with no Git worktree, branch or repository requirement. Git stays
optional at import/export boundaries; Gas Town's worktree machinery is not adopted.

The README's hook terminology is less precise than current source. The
[hook writer](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/sling_helpers.go#L888-L895)
is now a no-op because work status and assignee already carry the authoritative
relationship. Locust should likewise derive current work from durable attempts
and session bindings instead of inventing another mutable current-task pointer.

## 2. Publication, integration and cleanup need different evidence

The active `gt mq post-merge` command verifies a merge request's submitted commit
is reachable from its actual push destination before closing work and deleting
branches. It rejects absent target or submitted commit information. See
[post-merge proof](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/mq.go#L583-L619).
This is stronger evidence than a worker's completion message or a successful local
merge command, although Git ancestry is not proof that a contribution's intended
semantics survived integration.

**Apply:** expose separate Locust outcomes for attempt ended, proposal durably
published, candidate integrated at revision R, and local checkout updated to R.
After an uncertain write, reconcile the same idempotency key and event/object IDs.
Do not publish a fresh candidate from a changed workdir just because a response
was lost. A task's generic completion status cannot prove its code is in the tree.

Gas Town's `done` flow retains completion and merge-request identity before session
retirement, but some metadata write failures only produce warnings. See
[completion handoff](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/done.go#L1887-L1975).
Borrow the lifecycle separation, not a blanket guarantee that this path is atomic.
The first Locust workflow persists the checkout/base, exact request/candidate and
publication receipt for interrupted operations. Extending that record into automatic
adapter retirement/handoff is deferred. Distinguish durable local storage from
confirmed replication; neither a queued send nor an agent exit proves peer receipt.

## 3. Use one checkout disposition classifier

Gas Town has a pure classifier for the facts used by status, recovery and reuse:
live work, dirty files, unpublished commits, pending merge requests and failed
inspection. Unknown state prevents destructive reuse. See
[classifier inputs and outputs](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/polecat/workstate.go#L13-L55)
and [blocking conditions](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/polecat/workstate.go#L104-L189).
Its [tests](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/polecat/workstate_test.go#L17-L114)
explicitly cover terminal tasks whose files/publication still need preservation.

**Apply:** add one local disposition function to Locust's workspace lifecycle,
shared initially by status and update, and reused by later adapter lifecycle or
cleanup work. Report active, dirty, publication pending, recovery needed and unknown independently
where several conditions hold. Completing a task must not erase these blockers.

Locust already preserves unknown process state and separates launch metadata from
active ownership in [managed adapters](../crates/locust-adapter/src/managed.rs).
Retain explicit checkout identity and shared base revision when binding an agent's
work. Deeper lifecycle automation can follow; it must not introduce a second
supervisor or treat persisted PIDs as launch authority.

## 4. Integration needs a durable worklist and exact candidates

Gas Town's Refinery gives completed worker output a visible integration workflow.
However, the live manager
[starts an agent in a tmux session](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/refinery/manager.go#L194-L203).
The [patrol formula](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/formula/formulas/mol-refinery-patrol.formula.toml#L354-L466)
instructs that agent to rehearse a merge and run configurable checks; tests can
be disabled or unconfigured. A later
[merge/push step](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/formula/formulas/mol-refinery-patrol.formula.toml#L610-L643)
merges branch references again. That instruction sequence does not establish a
signed attestation for the identical candidate Locust will accept.

The separate
[integration-branch landing CLI](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/mq_integration.go#L624-L723)
performs a temporary-worktree merge, optional configured tests and push, with
recovery for an already-merged branch. Its
[cleanup](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/mq_integration.go#L740-L757)
also checks destination reachability before closing the epic.

**Apply:** derive an integration worklist from durable proposals and decisions,
with explicit missing-content, awaiting-evidence, stale-base, conflict, ready,
integrated and disputed observations. Persist any authoritative choice; local
queue order itself grants no authority. Required checks must bind to the exact
composition, parent revision and policy, and be enforced by the Rust evaluator.
On a changed head, create/review a new candidate; do not replay an agent's earlier
claim that tests passed.

Gas Town also contains a [batch/bisection engine](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/refinery/batch.go#L211-L303)
and [test definitions](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/refinery/batch_test.go).
Source search found `ProcessMRInfo`/`ProcessBatch` outside tests only at their
declarations, while manager startup explicitly launches the agent workflow.
Therefore this review does not establish that the batch engine drives the default
live queue. Bisection is a later optimization to evaluate, not a demonstrated
throughput advantage or a first-version dependency for Locust.

## 5. Start dependent work from integrated results

The [integration-branch workflow](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/docs/concepts/integration-branches.md#L18-L65)
groups related changes and starts later workers from already integrated sibling
work. This is a useful product pattern, even though additional branches are not
needed for Locust's first canonical tree.

Locust currently has publication, review, completion and selection prerequisites
in [EvidenceKind](../crates/locust-proto/src/organization.rs), resolved by
[flow evaluation](../crates/locust-core/src/goal/flow.rs). None explicitly means
that a contribution was incorporated into a workspace revision.

**Follow-up, deferred from the first version:** add an explicit integration
prerequisite for file-dependent stages and pin their input revision to its durable
witness. Choose a descendant containing
all required integrations, or keep the stage pending; never read an unrelated
moving head. Record the chosen revision with the task/attempt context. Existing
research-only completion semantics remain useful and should not be changed into
a requirement for code integration.

For atomic multi-task changes, compose several source contributions into one
reviewed proposal and integrate once. Initially this uses the existing one-tree
design; named integration branches can wait for a concrete requirement.

## Boundaries to retain

- **Keep coordination separate from files.** Gas Town redirects worker Beads
  access to shared local state ([redirect implementation](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/beads/beads_redirect.go#L14-L55)).
  Locust already has its own signed replicated state; a checkout update must not
  replace that state or import machine-local grants/provider configuration.
- **Reuse the capability-adapter idea.** Gas Town describes agent resume, hook,
  instruction and readiness capabilities in an
  [explicit registry](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/config/agents.go#L57-L173).
  Preserve Locust's agent independence. Do not import its
  [permission-bypass defaults](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/config/agents.go#L231-L289)
  or make tmux part of the peer protocol.
- **Keep publish and integrate authority distinct.** Gas Town's proxy has
  [authenticated clients](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/proxy/server.go#L209-L229)
  and [restricted worker branch writes](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/proxy/git.go#L309-L359).
  This is useful prior art for narrow capabilities, not proof of complete goal or
  repository isolation. Locust must enforce its own scope, principal and host checks.
- **Do not substitute federation branding for authority.** Wasteland explicitly
  describes [Phase 1 local claims and absent trust enforcement](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/docs/WASTELAND.md#L16-L20)
  and [independent claims reconciled later](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/docs/WASTELAND.md#L193-L207).
  The [claim code](https://github.com/gastownhall/gastown/blob/649b832b7672bc7a2dbef26f5983aba6198b819b/internal/cmd/wl_claim.go#L36-L116)
  supports that narrower claim. Some trust evaluation code exists, but these claims
  are not distributed locks. Locust still needs its own epoch, fork and handoff rules.

## Resulting plan changes

The [plan](../docs/shared-file-tree-plan.md) applies the durable-state lessons to a
smaller Git-independent loop: shared head, ordinary-directory checkout, selected
proposal, integration and explicit update. Pending proposals are a derived view
of signed records. Exact write receipts, local status and interrupted-operation
recovery are in scope; a separate queue service is not needed.

Integration-based task prerequisites, automatic adapter handoff/retirement,
checkout cleanup and batch scheduling remain research recommendations for later
work. The first version does not adopt Gas Town's role hierarchy, branch/worktree
management, local database topology, agent-supervision runtime or unqualified
merge guarantees.
