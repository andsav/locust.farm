# Public manual

Status: built.

## 1. What the manual is

The public manual is the user guide in `docs/guide/`, starting at
[guide/overview.md](guide/overview.md). Its readers are people who run locust.farm and
the coding agents they use. The website renders it under `/docs/next/`, behind
the site password. Release bundles include it as `manual.tar`, and installation
keeps that file next to the binary.

The current manual is served under `/docs/next/`.

The engineering docs in `docs/` are not part of the manual. Guide links to them go
to the GitHub repository.

## 2. Where pages come from

[site.json](site.json) chooses what the manual contains. It has these parts:

- `track`, `label`, `repository` and `versions` (software label, API, protocol
  and formation schema).
- `pages`: one entry per guide page, with its source file, slug, title,
  description, sidebar group, order, audience and status badge. The status is one
  of `development`, `proposed`, `implemented` or `verified`. The sidebar shows the
  groups in the order they first appear.
- `routes`: subject routes, each `{slug, page, section}`. A subject route shows
  its whole parent page with a note that points to the section. Its canonical URL
  is the parent page.
- `artifacts`: JSON files served byte for byte under `/docs/next/reference/` and
  `/docs/next/examples/`. Their sources live in `docs/reference/` and
  `examples/formations/`.
- `sourceLinks`: the files outside `docs/guide/` that a guide page may link.

Two pages get generated sections appended to their Markdown at build time:

- `schema-reference` gets the offline formation commands, the formation root
  fields and the example downloads, from `organization.contract.json` and the
  schema.
- `runtime-reference` gets the local API and MCP operations, the CLI, the
  response variants, the signed event variants and the error codes, from
  `runtime.contract.json`.

Do not repeat these tables in the authored Markdown. The build fails if the
schema, the contract files, the example files and the versions in `site.json`
disagree.

## 3. Writing rules

- Link another guide page with a relative path, such as `concepts.md#goals`. The
  build turns it into `/docs/next/concepts#goals`.
- An anchor must match a heading written in the target page. Links to generated
  headings fail.
- A link to any other repository file must be listed in `sourceLinks`, or the
  build fails. It becomes a GitHub link to the current `main` branch. `../README.md` is
  always refused.
- Use plain inline links only. Reference-style links and link titles break the raw
  Markdown copy. Images cannot be published.
- Raw HTML shows as text.
- The H1 is the page heading and should equal the title in `site.json`. The page's
  table of contents lists H2 headings only.
- Each page needs more than 500 characters of text and at least three headings.

Heading ids are built like this:

1. Lowercase the text and delete every character outside `a-z`, `0-9`, spaces and
   hyphens. "locust.farm" becomes `locustfarm`.
2. Trim it, then replace each run of spaces with one hyphen.
3. Number repeats in page order: `name`, `name-2`, `name-3`. Generated sections
   count too.

A recipe is a `bash` fence whose first line is `# locust-doc-test: NAME`, where
NAME uses `a-z`, `0-9` and hyphens and is unique on the page. That marker text may
not appear anywhere else in a guide page. Recipes run in a new temporary
directory with `LOCUST_BIN` and `LOCUST_DOC_DIR` set. They may use `python3`
and `git`.

## 4. Checks

| Check | Run by | What it catches |
| --- | --- | --- |
| Manifest checks in `content.ts` | `npm test` and `npm run build`, when the module loads | Bad slugs, sources outside `docs/guide/`, missing files, bad artifacts, subject routes whose section heading is missing |
| `content.test.ts` | `npm test` | Short pages, broken guide links and anchors, unapproved or unsafe links, generated references that miss an operation, wrong values in `availability.json`, API and protocol numbers that differ from `crates/locust-proto`, and inventory slugs that do not resolve |
| `scripts/check-prerender.mjs` | `npm run build`, at the end | Pages without an H1, broken local links and anchors in the built site, raw Markdown or artifacts whose hashes differ from `index.json` |
| `scripts/check_formations.py` | CI | Exported contracts, schemas, examples and conformance vectors that differ from the binary's output |
| `scripts/check_documentation.py` | CI | Recipes that fail against the built binary |
| `scripts/check_docs.py` | CI | Broken local links in `docs/` and `research/`, and index files that miss a document or list it twice |

Run the recipes with
`python3 scripts/check_documentation.py --binary target/debug/locust --timeout 60`.

Two site tests read other docs word for word: the setup prompt in
[first-contact.md](first-contact.md) and the fixed prompt sentences in
[formation-prompt.md](formation-prompt.md).

## 5. Complete manual inventory

A site test checks that every slug in the second column resolves to a page with
real content.

| Group | Slugs | What the reader gets |
| --- | --- | --- |
| Introduction | `overview`, `architecture`, `status` | What locust.farm does, how the parts fit together, and what works today |
| Installation | `install`, `install/local-candidate`, `install/macos`, `install/linux` | How to install the preview or a package by hand, and what is known about Linux |
| Setup and removal | `install/onboarding`, `install/verify`, `install/fresh-state`, `install/remove` | How to connect agents, check that setup worked, start with a new data directory and remove locust.farm |
| First collaboration | `quickstarts/two-local-agents`, `quickstarts/invite-a-person` | How to create or join a goal, admit members, grant permissions and see the first shared result |
| Working with code | `quickstarts/share-a-snapshot`, `quickstarts/contribute-and-review`, `quickstarts/apply-a-patch` | How to share exact files, publish and review a contribution, and apply a chosen patch safely |
| Core model | `concepts/goals-tasks`, `concepts/participants-roles`, `concepts/context-artifacts`, `concepts/attempts-contributions` | Goals, members, roles, tasks, attempts, contributions and shared files |
| Results and permissions | `concepts/decisions-completion`, `concepts/local-permissions`, `concepts/events-sync` | When a result counts, who picks one, the local permissions, and how each member's copy stays in sync |
| Formations | `organization/formations`, `organization/presets`, `organization/composition` | How to choose a formation, what the presets do, and how steps and task types work |
| Completion and change | `organization/completion`, `organization/lifecycle` | What makes a result count, what happens when several count, and how drafts, publishing and rule changes work |
| Writing formations | `authoring/with-your-agent`, `authoring/schema`, `authoring/examples` | How to have your agent draft, check, explain and publish a formation, and the schema and examples |
| Formation problems | `authoring/diagnostics`, `authoring/testing`, `authoring/custom-patterns` | How to fix validation errors, test a formation and combine the rules locust.farm supports |
| Polaris | `polaris/overview`, `polaris/visual-authoring`, `polaris/round-trip`, `polaris/observe-work` | What Polaris is and why this manual does not describe it |
| Coding agents | `agents/overview`, `agents/codex`, `agents/claude-code`, `agents/pi`, `agents/droid`, `agents/other-harnesses` | How to connect Codex, Claude Code, pi, Droid or another agent, and what each setup writes |
| Agent sessions | `agents/managed-sessions`, `agents/authoring-contract` | How to run an agent under locust.farm, resume it, see its pending work, and find the formation tools |
| Sharing | `sharing/visibility`, `sharing/snapshots`, `sharing/membership` | What members can read, which files are shared, and how invitations and removal work |
| Trust and retention | `sharing/trust`, `sharing/retention` | Credentials, encryption, network metadata, and why removing a member cannot erase their copies |
| Operations | `operations/services`, `operations/offline-recovery`, `operations/conflicts` | How to run the daemon as a service, restart and reconnect, and handle conflicting records |
| Recovery and diagnostics | `operations/cancellation`, `operations/diagnostics`, `operations/backup-recovery` | How to cancel work, collect diagnostics, and what backup support exists |
| Command reference | `reference/cli`, `reference/local-api`, `reference/mcp` | Every command, local API operation and MCP tool |
| Contract reference | `reference/formation-schema`, `reference/events`, `reference/errors`, `reference/configuration`, `reference/formats`, `reference/protocol` | The formation schema, events, error codes, configuration, file formats and version numbers |
| Help | `troubleshooting`, `faq`, `glossary`, `release-notes` | Fixes for common problems, answers to common questions, terms and changes |

## 6. Build and site implementation

`sites/locust.farm/src/lib/docs/content.ts` reads `docs/site.json` and the files
it names from the repository at build time, so the build needs a Git checkout. It
records the source commit and whether the working tree had changes.

The build prerenders these routes:

| Route | Content |
| --- | --- |
| `/docs` | Landing page with the page list and search |
| `/docs/next/SLUG` | One page for each page and subject route |
| `/docs/next/raw/SLUG.md` | The page's Markdown with links rewritten |
| `/docs/next/index.json` | Source commit, versions, routes, artifacts, and each page's headings, text and SHA-256 |
| `/docs/next/reference/FILE.json`, `/docs/next/examples/FILE.json` | The artifact files |

Search loads `index.json` when the reader first uses the search box, and matches
titles and page text. Reading and navigation work without JavaScript.
Unknown slugs return 404. `/sitemap.xml` lists every page and subject route.
`/llms.txt` is built from the setup guide's data in `src/lib/onboarding/`, not
from the manual.

[build_release.py](../scripts/build_release.py) packs `manual.tar` for a release.
It holds `docs/site.json`, every page, every artifact, every `sourceLinks` file
and `LICENSE`, plus a `manual.json` record of the source commit, versions and file
hashes. The builder stops if the API or protocol in `site.json` differs from the
binary's.
