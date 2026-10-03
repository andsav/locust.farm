# Documentation

Keep project documentation, accepted design decisions, and implementation plans
here. Exploratory work belongs in [`../research/`](../research/README.md).

Give each document a clear status, keep it aligned with the implementation, and
link to the research behind decisions. Preserve superseded decisions with a
supersession note rather than deleting their rationale.

Index each tracked Markdown document below using a relative inline link. After
staging new documents and index updates, run `python3 scripts/check_docs.py` from
the repository root to check coverage and local link paths.

## Index

- [Locust implementation plan](implementation-plan.md): proposed architecture, coordination and task contracts, Locust-owned Rust client adapters, one-prompt installation, milestones and acceptance tests.
- [Implementation plan review](implementation-plan-review.md): adversarial review of the proposed plan with prioritized objections, strengths and pre-implementation gates.
- [Second review response](implementation-plan-review-response.md): adopted plan refinements, qualified simplifications and claims not adopted.
- [Release evidence ledger](release-evidence.md): October 4 gate status and reproducible evidence requirements; no runtime checks recorded yet.
