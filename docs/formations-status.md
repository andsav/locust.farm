# Formation implementation status

Updated 2026-10-04. The development runtime is API 5 / protocol 5, store schema 5.
It implements the [accepted organization direction](formations.md) and
[signed semantics](formations-semantics.md). Unsupported homes, definitions,
events and peers are refused; there is no migration or alternate old runtime.
The [published terminal preview](public-preview-release.md) remains API 4.

## Source and enforcing checks

| Behavior | Implementation and executable checks |
| --- | --- |
| Offline authoring and private catalog | [Organization tests](../crates/locust-core/src/organization/tests.rs), [catalog tests](../crates/locust-core/src/organization/catalog/tests.rs), [CLI tests](../crates/locust/tests/formations.rs) |
| Separate governance and work rules | [Goal evaluation](../crates/locust-core/src/goal/fold.rs), [goal tests](../crates/locust-core/src/goal/tests.rs), [replica tests](../crates/locust-core/src/node/replica_tests.rs) |
| Taskless findings and independent attempts | [Public Engine journeys](../crates/locust-core/tests/organizations.rs) |
| Exact eligible completion evidence and scoped selection | [Goal tests](../crates/locust-core/src/goal/tests.rs), [selection projection](../crates/locust-core/src/goal/projection.rs), [read regressions](../crates/locust-core/src/node/tests/content.rs) |
| Parent authority attenuation and pinned child rules | [Delegation](../crates/locust-core/src/goal/delegation.rs), [goal tests](../crates/locust-core/src/goal/tests.rs) |
| Atomic events, delivery records and request receipts | [Commit path](../crates/locust-core/src/node/commit.rs), [failure tests](../crates/locust-core/src/node/tests/failure.rs), [delivery tests](../crates/locust-core/src/node/tests/delivery.rs) |
| Local permission and session boundaries | [Access checks](../crates/locust-core/src/node/access.rs), [Engine acceptance tests](../crates/locust-core/tests/organizations.rs) |
| Exact shared context and contribution sources | [Context tests](../crates/locust-core/src/node/tests/context_views.rs), [provenance tests](../crates/locust-core/src/node/tests/provenance.rs) |
| Separate-goal explicit export | [Subgroup acceptance](../research/subgroup-qualification.md), [export tutorial](guide/sharing.md) |
| Fresh-state persistence and refusal | [Store tests](../crates/locust-store/src/tests.rs), [setup tests](../crates/locust/src/installation/setup/tests.rs) |
| Reviewed local patch application | [Application tutorial](guide/apply.md), [workspace tests](../crates/locust-workspace/src/lib.rs) |
| Public views with explicit consent | [Farm guide](guide/farm-publication.md), [farm checks](../scripts/check_farm.py) |
| Model safety, witnesses and deliberate mutations | [Current model map](../research/tla/organization.md), [case registry](../research/tla/cases.json) |

These links identify source and runnable checks. Test presence does not establish
that a particular binary, client account, installation or network has passed them.

## Reproduce the source checks

Run the Rust gates in [AGENTS.md](../AGENTS.md), the generated-contract checks in
[check_formations.py](../scripts/check_formations.py), and the executable manual
recipes in [check_documentation.py](../scripts/check_documentation.py).
[The model guide](../research/tla/README.md) explains finite configurations and
completion criteria. Extended, ignored or external tests need their own run records.

## Qualification limits

The [release record](release-evidence.md) distinguishes the published preview,
current source, native clients, real-provider experiments, public farm rehearsal
and paired performance measurements. The retained experiments name their exact
candidate revisions; they do not qualify all subsequent API/protocol changes.

Linux runtime/installation, physical-network recovery, interactive client
approvals, worker confinement and unattended scheduling each require separate
identified evidence. A local claim generation is not a distributed exclusive
reservation. Optional visual integrations use the same Locust contract; their
availability and packaged-native behavior are separate from this repository's
source checks.
