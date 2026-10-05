# Public manual coverage contract

This is the maintained reader-outcome inventory for Locust's public manual.
[docs/site.json](site.json) selects canonical articles, reference assets and routes;
[the reader index](guide/README.md) is the entry point. The site coverage test
checks every subject below against substantive canonical content.

## 5. Complete manual inventory

The slugs below are relative to the selected documentation version. They define
required coverage, not a requirement to create one tiny page for every concept.
Merge closely related articles if it improves reading, while retaining the
coverage and stable routes recorded in the manifest. Each procedure names its
prerequisites, actions, expected observations, relevant permissions, and recovery
path. Each reference names its version and authoritative source.

| Group | Subject slugs | Required reader outcome |
| --- | --- | --- |
| Introduction | `overview`, `architecture`, `status` | Understand the installation-to-result journey, daemon/harness/transport/optional Polaris boundary, current publication and qualification state |
| Installation | `install`, `install/local-candidate`, `install/macos`, `install/linux` | Choose a supported route, supply actual trust/prerequisite inputs, verify the candidate, and understand platform evidence boundaries |
| Onboarding and maintenance | `install/onboarding`, `install/verify`, `install/fresh-state`, `install/remove` | Use reviewed profile/workspace/service choices, distinguish setup from skill/tool readiness, initialize explicitly selected fresh state, reject unsupported existing state, and remove software through its supported procedure |
| First collaboration | `quickstarts/two-local-agents`, `quickstarts/invite-a-person` | Create or join a goal, select a valid arrangement, bind actual participants, grant local work intentionally, and observe the first shared result |
| Working with code | `quickstarts/share-a-snapshot`, `quickstarts/contribute-and-review`, `quickstarts/apply-a-patch` | Select exactly what is shared, publish a contribution, inspect evidence/review, and apply an explicitly chosen patch with base/dirty-work protection |
| Core model | `concepts/goals-tasks`, `concepts/participants-roles`, `concepts/context-artifacts`, `concepts/attempts-contributions` | Understand optional tasks/roles, independent attempts, unattached findings, immutable artifacts, and identity without assuming a universal coordinator |
| Outcomes and authority | `concepts/decisions-completion`, `concepts/local-permissions`, `concepts/events-sync` | Distinguish submission, approval, criterion satisfaction, selection, closure, local application, and the age/completeness of a replica's view |
| Organization | `organization/formations`, `organization/presets`, `organization/composition` | Choose/pin a formation, understand the shipped presets, specialize tasks within parent authority, and compose optional dependencies or child work |
| Completion and change | `organization/completion`, `organization/lifecycle` | Read exact evidence/judgment requirements, understand multiple qualifying outputs, and distinguish draft, published definition, instance, authorized semantic revision, and reopen within the current model |
| Agent authoring | `authoring/with-your-agent`, `authoring/schema`, `authoring/examples` | Turn a plain-language request into a draft, validate, explain defaults/effects, publish locally, and instantiate deliberately using the real schema and examples |
| Authoring diagnostics | `authoring/diagnostics`, `authoring/testing`, `authoring/custom-patterns` | Correct syntax/semantic/binding/capability errors, test representative event traces, and combine supported rules without inventing executable policy |
| Polaris | `polaris/overview`, `polaris/visual-authoring`, `polaris/round-trip`, `polaris/observe-work` | Understand actual desktop availability, edit the current shared contract visually, distinguish semantic/layout changes, refuse unsupported input without rewriting it, handle concurrent edits, and inspect fresh/stale goal state |
| Harnesses | `agents/overview`, `agents/codex`, `agents/claude-code`, `agents/pi`, `agents/droid`, `agents/other-harnesses` | Check transport/instruction/approval capabilities, follow version-specific configuration/refresh steps, and distinguish adapter preparation from qualified real-client use |
| Agent lifecycle | `agents/managed-sessions`, `agents/authoring-contract` | Understand exact native/Locust identity bindings, resume, pending work, cancellation, closed-client limitations, and machine-readable authoring discovery |
| Information sharing | `sharing/visibility`, `sharing/snapshots`, `sharing/membership` | Know what goal members can read, which exact files/history are shared, and what invitations, leave/removal, key epochs, and separate goals mean |
| Security and retention | `sharing/trust`, `sharing/retention` | Understand credentials/local grants, artifact trust, encryption and discovery/relay metadata, independent harness access, and why removal/withdrawal cannot erase learned copies |
| Operations | `operations/services`, `operations/offline-recovery`, `operations/conflicts` | Run supported service modes and interpret reconnect, restart, sleep/wake, missing content, conflicting drafts/attempts/decisions, stale bases, and dirty checkouts |
| Recovery and diagnostics | `operations/cancellation`, `operations/diagnostics`, `operations/backup-recovery` | Distinguish requested from observed stop, collect redacted diagnostics, preserve state, and use only supported backup/recovery procedures |
| Command and integration reference | `reference/cli`, `reference/local-api`, `reference/mcp` | Find exact commands, audiences/effects, flags, request/result shapes, transport behavior, pagination, errors, and version handling |
| Contract reference | `reference/formation-schema`, `reference/events`, `reference/errors`, `reference/configuration`, `reference/formats`, `reference/protocol` | Look up the current schema/API/protocol identifiers, normalization, defaults, unsupported-input errors, event/proof identity, configuration ownership, and wire constraints |
| Help | `troubleshooting`, `faq`, `glossary`, `release-notes` | Resolve symptom-specific failures, understand fresh-state setup and unsupported-input refusal, and find precise terminology and changes to the current product |

The preset guide covers the arrangements delivered by the main plan: Open
collaboration, Coordinator, shared pool with peer review, independent attempts,
review panels, and pipeline/handoff composition. Explain guarantees as well as
shape: a local claim generation is not a distributed reservation; reviewer
thresholds are not consensus on one winner; a handoff offer does not wake a
remote machine. Show the same short human explanation returned by the core
explainer beside each example.

The completion guide must include at least these contrasting examples:

1. A participant publishes an unattached finding in Open collaboration; no
   invented task acceptance or common code head is required.
2. A coordinator selects an exact submitted contribution.
3. A peer review satisfies a candidate's completion rule while another
   independently approved candidate remains visible.
4. An explicitly configured decision authority selects one output where the
   arrangement requires uniqueness.
5. A pipeline makes downstream work available after named evidence arrives,
   while local authorization and actual execution remain separate.

Sharing documentation must state that initial goal membership remains the read
boundary. Topics and roles organize attention and authority; they are not
private channels. Installation does not automatically share local files,
private chats, hidden reasoning, credentials, or unrestricted harness access.
Document actual local access boundaries without describing Locust as a sandbox
around another agent's existing tools.

The fresh-state article documents explicit selection of a new empty state
location and initialization under the current contract. Unsupported state,
definitions, events, or peers receive a clear error; do not offer a converter,
old-format reader, mixed-version daemon, or legacy command path. This is an explicit
setup procedure, not authorization to erase a person's existing directory.
Organization rule revisions within the supported model remain documented; they
do not imply schema-format migration or support for retired readers.

## 6. Build and site implementation

The [SvelteKit documentation implementation](../sites/locust.farm/src/lib/docs/content.ts)
parses the selected Markdown and manifest once for human and agent-readable
surfaces. Generated JSON references come from Locust; examples and executable
recipes are checked against the CLI. The [site checks](../sites/locust.farm/README.md)
cover content, routes, accessibility and prerendered assets. The matching
[packaged manual](packaging.md) is authenticated with the release.

Run [check_docs.py](../scripts/check_docs.py) for tracked Markdown links and indexes,
[check_formations.py](../scripts/check_formations.py) for reference drift, and
[check_documentation.py](../scripts/check_documentation.py) for tutorial behavior.
A local site build does not establish publication or a native client journey.
