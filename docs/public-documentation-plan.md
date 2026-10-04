# Public documentation implementation plan

Date: 2026-10-04. **Status: proposed implementation plan; no documentation system,
new public manual, hosting configuration, or deployment is implemented by this
document.** This is the public-documentation workstream of the
[organization blueprints implementation plan](organization-blueprints-implementation-plan.md).
It covers the whole Locust product, including installation, collaboration,
operations, agent integrations, and Polaris, as well as organization blueprints.

The proposed packages are O9a, the documentation foundation; O9b, the complete
manual and its qualification; and the documentation portion of O12, authorized
public release and live verification. Documentation grows alongside the runtime
packages. A few blueprint pages, a generated API table, or a collection of
placeholders does not complete this workstream.

## 1. Outcome and evidence boundaries

A person arriving at locust.farm can understand what Locust does, find the route
appropriate to their available software and agent, verify readiness, create or
join a goal, choose an organization, share selected material, contribute work,
and interpret results. They can recover from ordinary failures without guessing
which state is authoritative or which command changes their machine.

An agent can discover the same versioned instructions, schemas, examples, and
reference data without scraping navigation or guessing command/field names. A
Polaris user can understand the same model through the visual editor guide.
Locust remains independently usable when Polaris is unavailable.

Every published instruction must identify the applicable software/API/protocol
or blueprint-schema version. Separate these claims throughout the site:

- **Implemented:** behavior exists in the identified source.
- **Verified:** a named check exercised it under stated platform, client,
  artifact, account/model, and environment conditions.
- **Published:** an identified artifact or service is actually available at the
  advertised public address.
- **Proposed:** a design or future capability, unsuitable as present-tense
  installation or operating instructions.

These are independent facts. A successful source build does not establish a
downloadable release; a written Polaris guide does not establish native editor
integration; a rendered documentation page does not establish a deployed site.
No deployment or release authorization is implied by this plan.

## 2. Current implementation and material documentation debt

The following source was inspected while preparing this plan:

| Area | Existing source | Reuse or required change |
| --- | --- | --- |
| Site runtime | [package manifest](../sites/locust.farm/package.json), [Vite configuration](../sites/locust.farm/vite.config.ts), [root page options](../sites/locust.farm/src/routes/+layout.ts) | Existing SvelteKit 3/Svelte 5 project, TypeScript, Node tests, and prerendering; retain this application |
| Documentation entry | [docs route](../sites/locust.farm/src/routes/docs/+page.svelte) | Replace the explicit placeholder with a real documentation landing page |
| Navigation and visual system | [site navigation](../sites/locust.farm/src/lib/site.ts), [header](../sites/locust.farm/src/lib/components/SiteHeader.svelte), [tokens](../sites/locust.farm/src/lib/styles/tokens.css) | Reuse identity, colors, spacing, and shared navigation; add a reading-oriented docs layout |
| First-contact content | [start route](../sites/locust.farm/src/routes/start/+page.svelte), [guide data](../sites/locust.farm/src/lib/onboarding/guide.ts), [first-contact contract](first-contact.md) | Reconcile old source snapshots with current behavior before publishing new claims |
| Agent-readable entry | [plain-text generator](../sites/locust.farm/src/lib/onboarding/llms.ts), [llms route](../sites/locust.farm/src/routes/llms.txt/+server.ts) | Preserve the shared-content approach; add version-aware documentation discovery |
| Copy interaction | [copy component](../sites/locust.farm/src/lib/components/CopyPrompt.svelte), [clipboard tests](../sites/locust.farm/src/lib/onboarding/clipboard.test.ts) | Reuse truthful success/failure and manual-copy behavior for prompts and code |
| Site tests | [site tests](../sites/locust.farm/src/lib/site.test.ts), [guide tests](../sites/locust.farm/src/lib/onboarding/guide.test.ts), [llms tests](../sites/locust.farm/src/lib/onboarding/llms.test.ts) | Replace placeholder assertions; preserve useful content parity, routing, and approval checks |
| Contract sources | [operation registry](../crates/locust-proto/src/api.rs), [CLI arguments](../crates/locust/src/cli/args.rs), [MCP schemas](../crates/locust/src/mcp/schema.rs) | Generate references from actual metadata and schemas, adding missing export/parity support |
| Agent instructions | [packaged operating skill](../skills/locust/SKILL.md) | Publish the matching version's exact instructions rather than a separately maintained website copy |
| Existing operational material | [installation](installation.md), [onboarding](onboarding.md), [managed clients](managed-clients.md), [packaging](packaging.md), [release evidence](release-evidence.md) | Use as source evidence for reader-facing guides; preserve their detailed qualification boundaries |
| Checks and delivery | [repository docs checker](../scripts/check_docs.py), [CI](../.github/workflows/ci.yml), [release candidate workflow](../.github/workflows/release-build.yml), [site README](../sites/locust.farm/README.md) | Add site CI and a publication path; neither current workflow deploys the website |

The first-contact contract's current-status section describes an earlier
scaffold without daemon, CLI, MCP bridge, installer, or operating skill. The
onboarding documentation now describes implemented `up` behavior and local
macOS qualification. The website's guide and `llms.txt` retain the earlier
status. Reconcile these sources together: keep useful historical observations
labeled and linked to their dated logs, and replace obsolete present-tense
claims. Public distribution remains deferred; local qualification must not be
turned into a public installation promise.

The accepted [organization direction](organization-blueprints.md) is also
explicitly unimplemented. Until its runtime packages land, current coordinator
restrictions remain part of protocol-1 documentation. New blueprint examples
belong to a visibly identified development version until they run.

## 3. Canonical content and ownership

### 3.1 One manual, explicitly selected for publication

Use `docs/guide/` for canonical reader-facing Markdown and `docs/site.json` for
the publication manifest. Both paths are proposed additions. Continue keeping
architectural decisions, implementation plans, protocol history, and research
in their existing locations. Do not automatically publish all repository
Markdown as the product manual.

The manifest names each article's source path, stable slug, title, description,
navigation group/order, audience, applicable documentation track, and feature
status. Keep metadata in this one JSON file instead of introducing frontmatter
parsing. The prose remains ordinary Markdown readable in the repository.

Each public article has one canonical prose source. A page may explain a
protocol decision in user terms and link its engineering rationale, but must
not maintain a second copy of the same installation sequence or reference table.
If an existing document becomes the public source, move it deliberately and
retain an index or redirect note at its old location; do not silently break
repository links.

The existing checker recursively examines tracked Markdown under `docs/` and
requires every document to be indexed once in the root `docs/README.md`.
Preserve that contract initially: add an explicit root index entry for every
new guide article, including any nested README. Stage new pages and their index
entries before running `python3 scripts/check_docs.py`. There is no requirement
to rewrite the checker just to add the manual.

Add a separate site-content check for the manifest, route ownership, anchors,
assets, generated sources, and public links. The repository checker does not
validate heading fragments or understand public route mappings; passing it is
not a substitute for the site check.

### 3.2 References, examples, and support facts

Propose `docs/reference/generated/` for generated JSON contract data. Generate
command metadata, API operation names/effects/audiences, MCP descriptions/input
schemas, response/error reference data, and the blueprint schema from the
implemented contract. Render reference pages from those exports plus small
authored explanations. Do not hand-edit generated signatures or field tables.

`OPERATIONS` already joins API requests with names and operation metadata.
CLI fields and MCP schemas are not all derived from one complete schema today.
The exporter must account for that gap: use actual command builders and schema
functions, add response coverage, and test parity instead of claiming that the
existing registry already generates the entire reference. The public local-API
reference describes the actual transport/envelopes; do not label it an HTTP API
or generate an OpenAPI facade without an implemented HTTP contract.

Propose `examples/blueprints/` for canonical definitions and their expected
validation/explanation fixtures. Publish the exact tested files. Article code
blocks that demonstrate a shipped preset or schema construct must be extracted
from these files or checked against them. Keep placeholder identifiers visibly
marked. An illustrative pseudocode block must say so and must not look like a
runnable command.

Use a small reviewed availability record, proposed as
`docs/reference/availability.json`, to supply software publication and
qualification facts to the manual, `/start`, and `llms.txt`. Records name
platform/client versions, source or candidate identity, evidence scope,
verification date, and evidence links. This record summarizes reviewed evidence;
it does not infer support from passing a random test or replace the release
ledger. A changed field requires the corresponding retained evidence.

## 4. Versioning and routes

Use these proposed public addresses:

| Route | Purpose |
| --- | --- |
| `/docs` | Documentation landing page, current publication state, available versions, and entry paths |
| `/docs/next/<slug>` | Development manual tied to an identified source commit and visibly labeled unreleased |
| `/docs/versions/<release>/<slug>` | Immutable manual artifact for an exact software release |
| `/docs/versions/<release>/index.json` | Machine-readable page/reference/example inventory for that release |
| `/docs/versions/<release>/raw/<slug>.md` | Plain Markdown corresponding to each article |
| `/docs/versions/<release>/reference/...` | Versioned schemas, operation exports, operating skill, and reference pages |
| `/docs/versions/<release>/examples/...` | Exact checked example files and their provenance |
| `/llms.txt` | Concise site and version-aware agent entry index |

Use the same structure under `/docs/next` for development raw files and
references. The route manifest must enumerate these outputs; unknown slugs
return a genuine 404 rather than the landing page.

Build each release's documentation from its identified release source, alongside
the corresponding schemas/examples. Preserve immutable build artifacts and old
version prefixes when publishing subsequent releases. Do not duplicate every
historical manual into the active source tree. A mutable current-release pointer
may choose the default landing-page links; command instructions, schemas, and
agent-generated definitions can pin an immutable release URL.

Every page and machine index records the documentation release/source commit
and applicable API, protocol, and schema versions. These version numbers are
separate dimensions. An updated documentation site cannot make an old runtime
understand a new organization schema.

Until software is published, `/docs` points to clearly labeled development and
local-candidate instructions. A downloadable public installer is not invented
to make the quickstart look complete. An unimplemented feature may have design
documentation in the development track, but cannot appear as a working release
instruction.

Version switching keeps the equivalent slug when present, and otherwise explains
that the topic is unavailable in the selected version. Preserve redirected
article slugs deliberately; do not silently redirect a protocol-1 instruction
to a different protocol's command sequence.

## 5. Complete manual inventory

The slugs below are relative to the selected documentation version. They define
required coverage, not a requirement to create one tiny page for every concept.
Merge closely related articles if it improves reading, while retaining the
coverage and stable routes recorded in the manifest. Each procedure names its
prerequisites, actions, expected observations, relevant permissions, and recovery
path. Each reference names its version and authoritative source.

| Group | Proposed slugs | Required reader outcome |
| --- | --- | --- |
| Introduction | `overview`, `architecture`, `status` | Understand the installation-to-result journey, daemon/harness/transport/optional Polaris boundary, current publication and qualification state |
| Installation | `install`, `install/local-candidate`, `install/macos`, `install/linux` | Choose a supported route, supply actual trust/prerequisite inputs, verify the candidate, and understand platform evidence boundaries |
| Onboarding and maintenance | `install/onboarding`, `install/verify`, `install/upgrade-remove` | Use reviewed `up`/profile/workspace/service choices, distinguish setup from skill/tool readiness, upgrade/restart/remove without losing retained identity/data |
| First collaboration | `quickstarts/two-local-agents`, `quickstarts/invite-a-person` | Create or join a goal, select a valid arrangement, bind actual participants, grant local work intentionally, and observe the first shared result |
| Working with code | `quickstarts/share-a-snapshot`, `quickstarts/contribute-and-review`, `quickstarts/apply-a-patch` | Select exactly what is shared, publish a contribution, inspect evidence/review, and apply an explicitly chosen patch with base/dirty-work protection |
| Core model | `concepts/goals-tasks`, `concepts/participants-roles`, `concepts/context-artifacts`, `concepts/attempts-contributions` | Understand optional tasks/roles, independent attempts, unattached findings, immutable artifacts, and identity without assuming a universal coordinator |
| Outcomes and authority | `concepts/decisions-completion`, `concepts/local-permissions`, `concepts/events-sync` | Distinguish submission, approval, criterion satisfaction, selection, closure, local application, and the age/completeness of a replica's view |
| Organization | `organization/blueprints`, `organization/presets`, `organization/composition` | Choose/pin a blueprint, understand the shipped presets, specialize tasks within parent authority, and compose optional dependencies or child work |
| Completion and change | `organization/completion`, `organization/lifecycle` | Read exact evidence/judgment requirements, understand multiple qualifying outputs, and distinguish draft, published definition, instance, revision, reopen, and migration |
| Agent authoring | `authoring/with-your-agent`, `authoring/schema`, `authoring/examples` | Turn a plain-language request into a draft, validate, explain defaults/effects, publish locally, and instantiate deliberately using the real schema and examples |
| Authoring diagnostics | `authoring/diagnostics`, `authoring/testing`, `authoring/custom-patterns` | Correct syntax/semantic/binding/capability errors, test representative event traces, and combine supported rules without inventing executable policy |
| Polaris | `polaris/overview`, `polaris/visual-authoring`, `polaris/round-trip`, `polaris/observe-work` | Understand actual desktop availability, edit the shared contract visually, distinguish semantic/layout changes, preserve newer constructs, handle concurrent edits, and inspect fresh/stale goal state |
| Harnesses | `agents/overview`, `agents/codex`, `agents/claude-code`, `agents/pi`, `agents/droid`, `agents/other-harnesses` | Check transport/instruction/approval capabilities, follow version-specific configuration/refresh steps, and distinguish adapter preparation from qualified real-client use |
| Agent lifecycle | `agents/managed-sessions`, `agents/authoring-contract` | Understand exact native/Locust identity bindings, resume, pending work, cancellation, closed-client limitations, and machine-readable authoring discovery |
| Information sharing | `sharing/visibility`, `sharing/snapshots`, `sharing/membership` | Know what goal members can read, which exact files/history are shared, and what invitations, leave/removal, key epochs, and separate goals mean |
| Security and retention | `sharing/trust`, `sharing/retention` | Understand credentials/local grants, artifact trust, encryption and discovery/relay metadata, independent harness access, and why removal/withdrawal cannot erase learned copies |
| Operations | `operations/services`, `operations/offline-recovery`, `operations/conflicts` | Run supported service modes and interpret reconnect, restart, sleep/wake, missing content, conflicting drafts/attempts/decisions, stale bases, and dirty checkouts |
| Recovery and diagnostics | `operations/cancellation`, `operations/diagnostics`, `operations/backup-recovery` | Distinguish requested from observed stop, collect redacted diagnostics, preserve state, and use only supported backup/recovery procedures |
| Command and integration reference | `reference/cli`, `reference/local-api`, `reference/mcp` | Find exact commands, audiences/effects, flags, request/result shapes, transport behavior, pagination, errors, and version handling |
| Contract reference | `reference/blueprint-schema`, `reference/events`, `reference/errors`, `reference/configuration`, `reference/compatibility`, `reference/protocol` | Look up normalization, defaults, unsupported constructs, event/proof identity, errors, configuration ownership, wire constraints, and compatibility rules |
| Migration and help | `migration/protocol-1`, `migration/blueprints`, `troubleshooting`, `faq`, `glossary`, `release-notes` | Understand the chosen upgrade path, preserve legacy goals/data, resolve symptom-specific failures, and find precise terminology and release changes |

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

Migration articles follow the main plan's resolved D3 decision. Under the
proposed side-by-side baseline, explain separate homes/runtimes, selected
artifact export/import, new goal/principal bindings and re-established grants.
Do not document an in-place conversion or mixed-version daemon until it exists.

## 6. Build and site implementation

### 6.1 Keep the existing SvelteKit application

Add a shared documentation build module and a catch-all documentation route,
proposed under `sites/locust.farm/src/routes/docs/[...slug]/`. Use the manifest to
enumerate prerender entries, load the correct article/reference output, and
generate navigation/TOC/search metadata. Unknown paths must not read arbitrary
repository files. The [SvelteKit prerender documentation](https://svelte.dev/docs/kit/page-options#prerender)
describes the build model; the initial prototype must confirm it against the
site's pinned dependency set.

Use an established Markdown parser at build time rather than writing a new
parser or introducing another website framework. The proposed candidate is
`markdown-it`: its [official usage](https://github.com/markdown-it/markdown-it/blob/master/README.md)
uses ordinary JavaScript rendering, and its
[package metadata](https://github.com/markdown-it/markdown-it/blob/master/package.json)
provides ESM and TypeScript exports. Select and lock an actual release only after
the O9a one-page prototype passes with this site's Node/SvelteKit setup. These
upstream observations do not prove this repository's compatibility.

Disable raw HTML, keep code escaped, validate link schemes, and derive heading
IDs deterministically. No arbitrary Svelte/JavaScript execution inside Markdown,
remote code includes, or runtime rendering of untrusted agent content is needed.
If richer examples need an interactive component, implement it as an explicit
reviewed site component using checked data, not executable Markdown.

Generate build inputs into an ignored directory. Wire development, checking,
and production build commands to refresh them deterministically. A site build
reads reviewed committed contract exports and should not require a Rust rebuild
for a prose/layout-only change. CI separately regenerates contract data from
Rust and fails when committed exports drift.

### 6.2 One parse supplies every surface

The build must derive these outputs from the same article and manifest:

- HTML article and stable heading anchors.
- Sidebar, breadcrumbs, previous/next links, and page TOC.
- Raw Markdown with public/version-correct links.
- Search entries containing page/heading text and status/version metadata.
- Sitemap, canonical URL, title, description, and machine inventory entry.
- Links to exact source commit, applicable contract exports, and examples.

Resolve repository-relative links through the manifest. Mapped public articles
become versioned site URLs; approved source/evidence links become commit-pinned
repository URLs. Detect missing assets and links to internal-only material
rather than quietly publishing broken relative paths. Generated references use
the same navigation and version context as prose.

### 6.3 Reading, search, and accessibility

Reuse the site header and design tokens. Add a documentation layout with grouped
navigation, a readable article measure, breadcrumbs, a version/status control,
an optional TOC, source links, and code-copy controls. The current body token is
small monospaced text designed for brief marketing content; add a reading token
and verify long articles at normal and enlarged text sizes. Do not redesign the
homepage as a side effect.

Keep long articles free of the animated swarm background. Main content,
navigation, version links, and code remain readable without JavaScript. Reuse
the existing clipboard failure/manual-selection behavior; never claim copy
success before the browser confirms it.

Use a generated local search index, loaded when search is used. Search covers
titles, headings and body text, preserves exact command/error matches, and shows
the version/status of each result. Begin with a small deterministic search
implementation over this index; profile the real manual before adding a search
service or index library. Provide a useful no-results state and a browsable
index without JavaScript. Do not silently mix released and proposed behavior in
results; let readers deliberately search another version.

Verify keyboard operation, skip-to-content, visible focus, mobile navigation,
heading hierarchy, link names, screen-reader status announcements, contrast,
text zoom, reduced motion, and code/table overflow. A graph or screenshot needs
text explaining the same rules and outcomes. Diagram layout is presentation,
not the executable contract. Polaris screenshots must identify the verified
app/build and be refreshed when the documented interaction changes.

## 7. Agent-readable documentation

Extend `/llms.txt` into a compact entry index, retaining the existing shared
first-contact instructions. Link the applicable version inventory, installation
status, authoring contract, schema, examples, operating skill, and references.
Do not require every agent to consume the entire manual or a large generated
prompt before using Locust.

For each version, publish:

| Artifact | Source and contract |
| --- | --- |
| Documentation inventory | Manifest-derived `index.json` with page descriptions, source commit, status, raw URLs and content hashes |
| Raw articles | Same canonical Markdown as human pages, with correctly resolved versioned links |
| Blueprint schema | Export of the actual normalized definition shape, schema version and capability vocabulary |
| Blueprint examples | Exact validated files, expected effective-rule explanations, and available conformance fixtures |
| CLI/API/MCP reference data | Exported command and operation metadata, input/output/error definitions, transport and effect distinctions |
| Operating instructions | Exact release-matching packaged `skills/locust/SKILL.md` |
| Authoring instructions | Concise discover/read/draft/validate/explain/diff/publish/bind sequence using actual operations and structured diagnostics |

An optional complete plain-text manual can be generated later from the same
inventory; individual retrieval remains the primary path. Serving Markdown or
JSON uses the correct content type, stable URLs and version metadata, without
client-side execution or authentication for public documentation.

Explain schema versus semantic validation: valid JSON shape does not prove that
a rule is supported, bindings exist, local permissions are present, or a unique
decision can be finalized. Agents use the core validator and effective-rule
explainer for definition semantics, and contextual instance/action-readiness
responses for current bindings, permissions and availability; offline validation
cannot observe those local runtime facts. Unsupported behavior gets the same diagnostic
in CLI, MCP, Polaris, and documentation examples.

Keep secrets and local identifiers out of public fixtures. Example keys must be
explicit test data; no invitation ticket, owner credential, account identifier,
private path, or raw production transcript belongs in a generated site artifact.

## 8. Work packages and dependencies

### O9a — Documentation foundation and accurate entry path

Dependencies: current site source; D9 content/version decision; initial O1
contract shape for a representative example. Full runtime implementation is not
required to build the documentation shell.

1. Reconcile first-contact and installation facts against current implementation
   and retained qualification records. Update the source contract, guide data,
   `/start`, and `llms.txt` together; keep unpublished routes explicitly unavailable.
2. Add canonical content directory, publication manifest, availability record,
   and ownership/index checks.
3. Prototype one tutorial, one conceptual article, one generated reference page,
   and one downloadable blueprint example through the existing production build.
   Lock the renderer only after this passes.
4. Build the docs layout, dynamic-route enumeration, link/anchor mapping,
   machine inventory/raw outputs, initial search and version navigation.
5. Add reference-export plumbing and parity tests with O1/O7 rather than writing
   separate website schema/command definitions.
6. Add a site CI job, pin the Node version, use `npm ci`, and run all required
   site checks from `sites/locust.farm/`.

Exit evidence: a reproducible production artifact containing the representative
pages and exact machine-readable equivalents; manifest/link/anchor tests;
working no-JavaScript reading and keyboard navigation; all npm checks passing;
and reconciled entry claims with evidence links. This establishes the foundation,
not the complete manual or a public deployment.

### O9b — Full manual and executable documentation qualification

Dependencies: each article's corresponding runtime/authoring package; O7
CLI/MCP/skill contracts; O8 integration/migration work as defined in the main
plan; O10 Polaris implementation for its actual interaction guide; O11 evidence
for final qualification claims. Content drafting proceeds before all dependencies
finish, but working instructions cannot outrun the implementation.

1. Complete the inventory in section 5 with concrete procedures and accurate
   current limits. Have the owner of each contract review the relevant articles.
2. Generate full command/API/MCP/schema references, including responses, effects,
   errors and version behavior. Make regeneration drift a CI failure.
3. Validate every published blueprint and shared source snippet using the real
   core validator; check explanation outputs against the expected semantics.
4. Execute the quickstarts against identified candidates in disposable homes,
   profiles and workspaces. Reuse the existing installation/client/workspace
   qualification machinery where appropriate rather than inventing a mock
   documentation-only path that bypasses the real product.
5. Verify failures as well as success: stale draft, unsupported rule, unavailable
   authority, denied local action, missing content, offline participant, dirty
   apply and incompatible version must lead to documented observations/remedies.
6. Add actual Polaris authoring/round-trip/inspection instructions and diagrams
   only after matching browser and native integration evidence is available.
7. Review the full production build for readability, mobile use, search quality,
   accessibility, source/version clarity and agent retrieval.

Exit evidence: every inventory requirement has substantive content; no required
page is a stub; all examples match the candidate and schema; tutorials carry
specific verification scope; all public support claims have retained evidence;
and human/agent journeys find the correct version and reach observable results.
Unverified platform behavior remains labeled and cannot be counted as a passed
qualification gate.

### O12 — Public artifact and website release

Dependencies: O9a/O9b passed; actual candidate and publication inputs; the main
plan's release decision and explicit authorization. The repository presently
has no configured website host or deployment procedure.

1. Establish the hosting target, domain/DNS/TLS ownership, build identity,
   credential location, preview path, deployment/rollback procedure, and
   preservation of immutable version prefixes. Choose the concrete SvelteKit
   adapter after the host is known; do not assume `adapter-auto` targets every
   static host.
2. Build the site/manual and contract exports from the exact identified release
   source. Record the artifact digest and preserve the artifact used in preview.
3. Verify download links, signature/trust instructions and compatibility against
   actual published software inputs. Site publication and software publication
   are separate operations whose cross-links must resolve before claiming the
   public install journey works.
4. Publish the approved artifact through the established path and verify it
   independently from the public origin.
5. Record public URLs, deployed commit/build identity, digests, checks and any
   remaining evidence boundaries in the release ledger. Retain a rollback that
   does not remove already published immutable documentation versions.

Live checks cover HTTPS, canonical/redirect behavior, representative and indexed
article routes, version navigation, real 404s, search assets, raw Markdown,
schema/operation/example downloads, `/start`, `/llms.txt`, and public software
links. Inspect actual bytes and version metadata, not only a green deployment
status. Local preview is useful evidence but cannot close this public gate.

## 9. Verification and continuous maintenance

The required site commands remain `npm run lint`, `npm run check`, `npm test`,
and `npm run build`. Run them in the site directory. Use `npm ci` in CI with a
pinned supported Node version; the current tests require Node 22.18 or newer.
Record the selected runtime version in the site instructions and CI together.

Add meaningful checks for:

- Manifest completeness, duplicate routes, valid version/status metadata,
  referenced source/assets, canonical URLs, and unknown-route behavior.
- Heading IDs and anchor links, Markdown-to-public link rewriting, escaped code,
  unsafe-link rejection, and raw/HTML content parity.
- Exact contract export coverage and drift; every exposed operation documented;
  no owner-only operation accidentally presented as an agent tool.
- Example validation, normalization/explanation equivalence, and documented
  unsupported examples producing the expected diagnostic categories.
- No split between `/start`, public status, agent instructions, compatibility
  pages, and their evidence-backed availability record.
- Production-build browser journeys: navigation/search/version switch, mobile
  layout, keyboard use, copy success/failure, and reading without JavaScript.
- A crawler over the built manifest routes, internal anchors and machine files.
  External-link availability is reported separately from deterministic local
  build failures; a transient external outage is not a schema regression.

Keep the existing Python documentation/index checks. If their behavior changes,
run `python3 -m unittest discover -s scripts/tests`. Rust contract/exporter changes
also require the repository's pinned-toolchain formatting, strict all-target
Clippy, and workspace tests. Prose-only or site-only work does not require a Rust
rebuild merely because the manual describes Rust code.

For every behavior change, update the canonical article, generated references
or examples, availability evidence when applicable, and release notes in the
same logical change. A release check verifies that the required pages and
artifacts apply to the exact candidate. CI cannot prove that prose is correct;
contract-owner review and executed reader journeys remain required.

Public search/navigation must keep proposed features visibly distinct. Remove
obsolete caveats when evidence changes, preserve useful historical qualification
records, and never turn a source observation into a broader runtime/support claim.
Report exactly which documentation, browser, candidate, native-client, and live
site checks ran. The workstream is complete only when the full manual is accurate,
usable by people and agents, versioned with the product, and verified at the
authorized public origin.
