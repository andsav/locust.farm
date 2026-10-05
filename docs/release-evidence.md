# Release evidence

The development source implements API 5 / protocol 5 with store schema 5.
The [published macOS terminal preview](public-preview-release.md) is the separately
identified API-4 artifact. A test of that package does not qualify newer source.

| Surface | Evidence | Boundary |
| --- | --- | --- |
| Public Apple Silicon preview | [Release identity and qualification](public-preview-release.md), [structured record](../research/evidence/public-preview-release-2026-10-04.json) | Exact published binary, package trust and download/install observations; broader client/platform readiness remains separate |
| Current organization runtime | [Implementation boundaries](formations-status.md), [signed semantics](formations-semantics.md), [core API tests](../crates/locust-core/tests/organizations.rs) | Source and deterministic/local-daemon checks; no implied current published artifact |
| Reproducible manual recipes | [Recipe checker](../scripts/check_documentation.py), [reader guides](guide/README.md) | Actual CLI/daemon/file assertions; does not launch models |
| Client experiments | [Qualification harnesses](client-qualification.md), [collaboration findings](../research/collaboration-followups.md) | Identified clients, permission modes and candidate versions; scripted providers and real providers are distinct |
| Live public farm rehearsal | [Rehearsal](../research/live-farm-demo.md), [structured record](../research/evidence/live-farm-demo-2026-10-04.json) | Four native clients and one local daemon; no two-machine or unattended-scheduling claim |
| Formal models | [Model map](../research/tla/organization.md), [checker](../scripts/check_tla.py) | Bounded model safety/reachability; not a Rust refinement or arbitrary-size proof |
| Performance | [Paired measurements](../research/performance-cost-pass.md) | Stated workloads and revisions; not current production capacity or provider billing |

## Record a verification result

Name the source revision, binary hash, platform, client versions and profiles,
authentication/provider mode, topology, commands, assertions and cleanup. Keep
failed and unrun cases visible. Sanitize credentials, invitation bytes, local
account paths and unrelated workspace content before retaining a report.

A successful compile, native client process, scripted-provider response,
physical-network exchange and public artifact fetch establish different facts.
Report only the boundary exercised by the check. Real-provider experiments need
explicitly selected accounts and spending; local fixtures cannot close that gap.

Use [availability metadata](reference/availability.json) for the currently
advertised release and [the status guide](guide/status.md) for reader-facing limits.
