# Completion, review and selection

**Status: implemented development semantics, exercised by signed replay tests.** Submission,
approval, criterion satisfaction, selection, closure and local application are
different observations. None means a remote process has stopped.

## Five concrete outcomes

1. **Open finding:** Bea publishes an unattached research artifact under open
   membership rules while administrator Ada is offline. No assignment, task
   acceptance or shared code head is required. Chen's independent finding also
   remains visible.
2. **Coordinator selection:** a task delegates selection to Chen. Bea submits
   exact contribution C1; Chen selects C1 under that task's pinned rule. This
   authority does not make Chen the decision-maker for unrelated open work.
3. **Peer review:** Dev, an eligible non-author reviewer, approves C1. Its rule
   is satisfied. A separately approved C2 can also qualify. Bea's own review
   does not count when author exclusion is configured.
4. **Unique choice:** where one output is required, an explicitly bound selection
   authority selects the exact candidate. Time, event hash and arrival order
   never choose a winner. If the authority is offline, only its decision waits.
5. **Pipeline readiness:** named C1 evidence satisfies a declared downstream
   dependency. The daemon materializes and durably delivers the configured
   child-task/handoff effect. Receipt, acceptance, local permission and observed
   execution start remain separate.

The CLI operations reflect these separate outcomes:

```sh
locust contribution publish --goal GOAL 'Exact finding'
locust completion declare --goal GOAL --subject CONTRIBUTION
locust review record --goal GOAL --subject CONTRIBUTION --verdict approve 'Review of these bytes'
locust contributions --goal GOAL
locust scope select --goal GOAL --subject CONTRIBUTION
```

Each mutation is available only when that exact subject's pinned rule and the
caller's local grants authorize it. The sequence is a command reference, not a
universal workflow: Open uses declaration and has no selector; peer review uses
eligible reviews and may also have no selector. In a configured selection scope,
replace a decision only with its current event as `--expected`. For task work,
`task show` returns the current round and `effective_rules_json`; `pending`
returns caller-specific review/start/delivery obligations.

## Count admissible evidence

Evaluate each review against its exact contribution, rule/round, authenticated
bindings, membership tenure, signatures and retained proof. Count one eligible
principal once; duplicate signatures do not add reviewers. Apply author exclusion
before threshold counting. A role label does not prove independent humans or
organizations.

A negative review is recorded evidence, not an implicit veto or a retraction of
positive evidence. Veto and mutable-vote behavior remain unsupported. A check
label meets only the configured predicate; it does not establish real execution
or independent verification by itself.

## Interpret pending and disputed states

Missing reviewer binding or ancestry means pending verification. Fetching the
proof can allow reevaluation; missing proof is neither a negative review nor an
invalid one. A conflicting authority successor halts its affected scope. It does
not silently pick a hash-order winner or stop all other work.

Later removal cutoffs or discovered authority forks can change an observed
outcome. Retain the earlier record and expose the changed status rather than
calling it irreversible finality. [Event evidence and recovery](recovery.md)
explains scope and author-log forks.

The [signed semantics](../formations-semantics.md) are the engineering
source for these rules. [Organization lifecycle](organization.md) explains which
revisions change future defaults and which need explicit active-round transitions.
