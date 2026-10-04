# Blueprint authoring on locust.farm

Date: 2026-10-04. **Status: research supporting the
[proposed blueprint authoring plan](../docs/blueprint-authoring-plan.md); nothing
is implemented. Probes and measurements are listed with their method; everything
else is source reading or inference.** It extends the
[organization blueprint research](organization-blueprints.md) to one question:
how a person authors a blueprint visually on the marketing site and hands it to
their agent.

Labels: **[V]** verified by reading source or running a command; **[I]**
inference or recommendation; **[R]** third-party report, not confirmed here.

## Later decisions and findings, 2026-10-04

This section supersedes the rest of the note where they disagree. The rest
records the first two rounds of research. Identifiers such as WD4 or WR10 refer
to earlier drafts of the plan, kept in Git history; the current
[plan](../docs/blueprint-authoring-plan.md) lists decisions in plain words.

**Owner decisions after the first rounds.**

- Checks run in TypeScript in the page, not as WebAssembly. A WebAssembly build
  of `locust-core` was the earlier recommendation; it did not compile for the
  browser (`getrandom` through `chacha20poly1305`) and was judged too heavy for
  what the page needs.
- Contract names changed to `task_types`, `task_type`, `decisions.finish` and
  `runner` (commit `74ec9fb`). Findings below that say `variations`, `closure`
  or `materializer` predate the rename.
- The page opens with six ways of working, each with an animated diagram. The
  canvas shows only stages; everything else is edited as properties. Nodes and
  the side panel keep Polaris's look in the site's colours and font.
- Starting a goal is left out of the prompt.

**Findings from the revision at `809bbbe`.** [V] unless marked.

- The offline inspection is about 1,100 lines of Rust: loader, 13 rule codes
  plus `invalid_json`, `duplicate_key`, `unsupported_version` and
  `invalid_structure`, normalization, explanation and diff. A TypeScript port of
  the parts the page needs is practical [I].
- Locust's decoder is wider than its exported schema: a root array such as
  `[2]` passes the version check because the check reads `schema_version` only
  from objects; structs accept positional arrays (`"work":[]`); tagged enums
  accept array forms; unit selectors accept and drop extra fields
  (`{"kind":"members","x":1}`); input and evidence kinds accept a map form. The
  port follows the schema, and the test cases stay inside it. These are worth
  fixing in Locust.
- Messages for `invalid_json` and `invalid_structure` come from `serde_json`
  and include its own float formatting and type names. Matching them exactly in
  TypeScript is possible but costly; the page shows its own plain message for
  those two codes and matches code, phase and path.
- Explanation lines format names with Rust's `{:?}`, which escapes some
  non-ASCII characters. Exact agreement is tested on ASCII names [I].
- `goal.create` takes inline `blueprint_json`, makes the creator the only
  member, needs the daemon-wide `manage_goals` grant, and needs each required
  input bound to a blob sealed with the goal's own key, which cannot exist
  before the goal does. Every role must be present in the binding, and
  `finish`, `selection` and `runner` roles need exactly one member.
- A stage's runner's daemon creates the stage's task and delivers it when the
  stage is ready; no agent process is started.
- The presentation record is opaque `data_json` with its own revision and is
  never part of a blueprint's identity.

## Question

The owner asked for a plan, 2026-10-04: someone who has Locust authors a
blueprint on a 2D canvas on the marketing site, using the xyflow surface from
Polaris, which will ultimately be shared and may be imported wholesale; they
copy a prompt containing the blueprint that instructs their agent to create it
through the daemon; and it is extremely easy to use. The sharing part was
superseded the same day: the owner chose a fork with no shared repository or
package (short answer 3; plan WD1).

The blueprint contract and agent operations are being built in a separate
worktree; the plan assumes they exist as the
[blueprint implementation plan](../docs/organization-blueprints-implementation-plan.md)
specifies, with names provisional until O1 freezes them.

## Short answers

1. **The prompt is the only path to the daemon.** The site is prerendered and
   static; the daemon speaks length-prefixed postcard over a local socket and
   MCP over stdio. A bridge would put authoring credentials in an untrusted
   browser channel, which D8 and O10.7 exclude. [V]
2. **"Wholesale" applies to the generic pieces, not the canvas.** The generic
   modules split cleanly from the 3,457-line `BlueprintCanvas.svelte`, which is
   Merak orchestration and stays in Polaris. [V]
3. **The site forks the canvas; nothing is shared.** A shared package would
   have needed its own repository, since npm cannot install a package from a
   Git subdirectory. The owner chose instead, on 2026-10-04, to fork the generic
   modules from Polaris into the site: the two will look similar, not the same,
   and Polaris is unchanged. About 315 forked lines need only site conventions
   and about 2,200 need adaptation; several shared Polaris pieces (slide-over
   camera, dock, frame bar, haptics, inline rename) have no use on the web and
   are not forked. [V for whole-file counts; the 2,200 includes estimated used
   parts of three files, I; owner decision]
4. **The browser can only match Locust by running Locust.** A JSON Schema check
   is structural. Semantic diagnostics, effective defaults, the explanation and
   the definition hash need the Rust code, which a WebAssembly build could
   provide. As the workspace is configured today that build fails to compile
   (getrandom), and its size and speed are unmeasured. [V/I]
5. **The blueprint prompt needs its own contract.** The entry prompt is one
   fixed sentence set whose tests ban backticks, pipes and "token"; a blueprint
   prompt carries a document and operation steps. [V]
6. **Friendliness comes from starting finished.** Open collaboration is the
   default and needs no drawing; three plain questions reach the common
   arrangements; the canvas rewards depth but is never required. [I]
7. **The hand-off's safety rests on Locust, not on delimiters.** Sentinel lines
   are a weak, public signal. What protects the person is that Locust checks the
   pasted bytes, that nothing beyond a private draft happens without a separate
   yes, and that the prompt forbids owner authority, which a shell-capable agent
   could otherwise reach. [V/I]

## Method and evidence boundary

- **Baselines.** Locust `main` at `6b9365f`, clean, one worktree. The local
  Polaris checkout `dreamcolor10` at `01d8aa3c4` (2026-10-04, remote
  `33CCFF/dreamcolor10`). The main plan's section 8 calls this repository
  `merak10` at `4704c2a`; the checkout examined is `dreamcolor10`. [V]
- **Reader reports.** Six focused readings: the contract as the editor's
  target; the Polaris canvas; Polaris authoring UX; the site; the agent and
  daemon surface; ecosystem facts. Each cited files and lines.
- **Concepts.** Three independent UX concepts, guided, canvas-native and
  round-trip, were written against the same evidence and scored (section 2).
- **Review.** The draft plan was reviewed through four lenses: a first-time,
  phone and assistive-technology user; engineering; contract fidelity; trust and
  safety. Section 3 records what was checked and what changed.
- **Spot checks during synthesis**, 2026-10-04, all [V]:
  - `npm view @xyflow/svelte`: `latest` 1.7.0, `next` 2.0.0-next.3, peer
    `svelte ^5.25.0`, dependency `@xyflow/system` 0.0.83, MIT;
    `npm view @dagrejs/dagre`: 3.1.1, MIT.
  - npm 10.9.7's bundled install docs and npm 12.2.0's published docs: a Git
    spec ends in `#<commit-ish>` or `#semver:<semver>`, with no subdirectory
    form; a dependency's `prepare` script runs after its devDependencies install.
  - Node 22.22.2 importing a `.ts` file from a package under `node_modules`
    fails with `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`.
  - `python3 scripts/check_docs.py` on `main` fails only on two links to the
    missing `organization-blueprints-semantics.md`.
  - Polaris: module line counts, `package.json` versions, the Galaxy pin check,
    `NodeCapsule.svelte`'s fixed 216×46 box, `isValidConnection` in the loop
    editor, the imports of `MeaningRows`, `MeaningRow` and `InspectorPanel`.
  - Site: installed versions, `CopyPrompt.svelte`, `clipboard.ts`, `site.ts`
    and its test, the CI workflow. Locust: `manage_goals` enforcement and its
    onboarding default, two-level CLI dispatch, MCP's owner refusal and tool
    names.
- **Probes during review**, 2026-10-04, all [V], run in a scratch directory
  outside both repositories:
  - `cargo tree --offline -p locust-proto -e features -i getrandom@0.4.3
    --target wasm32-unknown-unknown`, and getrandom 0.4.3's `src/backends.rs`.
  - A one-file Cargo project with `trim-paths = true` under `cargo +1.96.1`:
    "feature `trim-paths` is required".
  - Node 22.22.2: `JSON.parse` and `JSON.stringify` on a document with
    integer-like keys, a large integer, `1.0` and escapes; `JSON.stringify` on
    strings holding U+2028, U+2029, U+0085, U+202E, U+200B and a tag character;
    importing a module with `export enum`.
  - Python's `str.splitlines` on the same separators.
  - Contrast ratios computed from the site's token values.
  - The GitHub API for the three repositories, without credentials: all return
    404, so all are private (this served the lapsed shared-package question).
- **Measurements by the ecosystem reader** in a scratch Vite 8 project, not in
  the site: xyflow bundle and CSS sizes, validator bundle sizes, synthetic
  blueprint sizes and their compressed form. Labelled [V measured] below.
- **Not done.** No WebAssembly build (no `wasm32` target or `wasm-pack` on this
  machine); no xyflow inside SvelteKit 3; no paste test in a real harness in
  either direction; no user test; no test on a real phone; no O1 export exists
  on `main`, so every field name is a reconstruction. No file in either
  repository was changed.

## 1. Findings

### 1.1 The contract the editor targets

- The editor produces two documents: the semantic definition (the draft body an
  agent publishes) and a separate presentation document (name, description,
  layout, its own revision). Goal and task instances are never authored in the
  page. [V] (main plan section 4.1)
- Building blocks: selectors (member, specific identity, bound role, task
  creator, contribution author, author exclusion); work policies; evidence
  predicates; `all`/`any`/count composition; derived flow; outcomes; role and
  input slots; delegated task variations; shared documents with optional
  selection authority (D12); guidance as hashed but advisory text. [V]
- Identity is the hash of the normalized typed value in the canonical codec
  (postcard plus BLAKE3). A TypeScript reimplementation would drift; the page
  should compute it only by running Locust's code, or not at all. O1's exit
  requires that formatting and key-order changes keep the hash. [V/I]
- The loader refuses duplicate keys and out-of-range numbers. `JSON.parse`
  silently keeps the last duplicate and rounds large integers, so the page must
  not parse first. [V/I]
- Unknown newer constructs are preserved read-only, never dropped; unknown
  behaviour-bearing fields are refused at publish. Raw newer source is
  preserved read-only when it cannot be interpreted. [V]
- A specific authenticated identity is one of the selectors, so an imported
  definition can carry participants' public keys and give them authority. [V]
- M1 delivers Open collaboration and Coordinator; the other arrangements,
  reviews, thresholds and composition arrive in M2. A definition can be
  structurally valid and still need capabilities an M1 daemon lacks. [V/I]
- Unsupported in the first delivery: exclusive reservations (a shared pool keeps
  concurrent attempts as independent attempts, D5), negative or absence
  conditions, lease expiry and clock reassignment, majority over a changing
  electorate, daemon-authored effects and automatic launch, executable policy,
  mandatory DAGs, private topics, different-membership subgroups, permission
  widening, static cycles, unknown fields, old versions. Limits are never
  defaulted but are kept when authored. [V]
- Exactly-one selection and authoritative closure use a named scope-specific
  authority (D4, D12), so an editor must never substitute "any member" for a
  deleted role in those places. [V source; I consequence]
- The accepted direction binds a visual editor to: one contract, no graph for
  simple arrangements, the same validator and explainer, a lossless round trip,
  layout separate from semantics, preservation of the unknown, visible defaults,
  revision checks and reviewable diffs between human and agent edits, and
  diagram edges that are not execution edges. [V]
- O1.4 promises JSON Schema, an operation and type catalog, generated
  TypeScript types and example inputs. A diagnostics catalog with canonical
  sentences and alternatives, a presentation type, element IDs everywhere and
  explainer fragments are not promised; the plan requests them (WR1 to WR9).
  O7.4 promises semantic element IDs only "where available". [V]
- The availability record proposed by the documentation plan holds publication
  and qualification facts (versions, candidate identity, evidence scope), not
  per-capability availability. [V]
- Under D3 a later release that changes the contract refuses goals created by
  an earlier one, so advising a person to update Locust to match the website
  can strand their goals. [V source; I consequence]
- Open: whether optional tasks in Open collaboration get a default completion
  rule or must choose one; whether role and stage names are identifiers or
  presentation labels. [V: the plans leave both open]

### 1.2 What the browser can check

- "Valid JSON shape does not prove that a rule is supported" (documentation
  plan section 7). [V]
- The plan mentions no WebAssembly or TypeScript validator; only generated
  types and JSON Schema. [V]
- `locust-core` depends only on `locust-proto` and `serde`; `locust-proto` on
  `blake3`, `chacha20poly1305`, `ed25519-dalek`, `postcard` and `serde`. [V]
- **As configured, `locust-proto` does not build for `wasm32-unknown-unknown`.**
  The workspace declares `chacha20poly1305 = "0.11"` with default features,
  which enable its `getrandom` feature; through `aead` and `crypto-common` that
  pulls in getrandom 0.4.3, whose backend selection reaches a `compile_error!`
  stating that the `wasm32-unknown-unknown` targets are not supported by
  default and that the `wasm_js` crate feature may need enabling. Two remedies:
  enable `wasm_js` for the WebAssembly crate only, which imports
  `crypto.getRandomValues`; or put the pure contract behind a feature or crate
  boundary that does not link AEAD or signing. Either touches lane A's
  dependency table. [V]
- Byte-reproducible WebAssembly across machines is not shown by two builds on
  one machine. The release profile's `strip = true` and `panic = "abort"` do not
  remove absolute registry paths embedded in panic locations, Cargo's
  `trim-paths` is unstable on the pinned 1.96.1 [V], and `wasm-opt`'s version
  changes its output [I]. Path remapping with `--remap-path-prefix` and exact
  pins are the available controls. [V/I]
- Sizes of other Rust-to-WebAssembly tools: Ruff's playground 10.9 MB raw
  (3.8 MB gzip); Biome 46.9 MB; Typst 28 MB; an older Oxc parser 737 KB
  (257 KB gzip). A serde-based validator at `opt-level = "z"` with LTO and
  `wasm-opt` is estimated at 80 to 250 KB gzip. [V for the others; I estimate]
- JavaScript validators in Vite 8: Ajv runtime 36 KB gzip and needs
  `unsafe-eval`; Ajv standalone generated from a schema 3.56 KB gzip;
  `@cfworker/json-schema` 6.1 KB gzip. [V measured]
- Contextual bindings, local grants, installed capabilities, draft revisions
  and publication remain daemon-only in every design. [V]

### 1.3 The Polaris canvas surface

- Three dialects already share the generic pieces: the Merak step graph, the
  loop editor and the read-only context map. They share the fit bridge,
  viewport controls, dock, frame bar, camera helpers, align snap, haptics,
  capsule node and edge visual. [V]
- Classification: generic (about 940 lines), generic needing adaptation (about
  3,500), shared helpers (about 600 plus icons), tests of generic modules (about
  1,000 to 1,300), Merak-specific code that stays (about 24,000 including the editor
  page, depth and loop editor). [V counts; split estimated]
- Merak assumptions hidden in generic-looking code: the 216×46 box in at least
  four places; loop and back-edge bounds in the camera; Merak keys in the
  reconcile signature; run states in edge presentation and the edge visual;
  `.status-failed` in the capsule; the `merak10:` event and `blueprint-node`
  MIME type; the `polaris:blueprint-canvas:v2:` storage prefix; the return-node
  rule in connection handling. [V]
- SvelteKit 3 removed `$lib` (`module_removed_lib`) and `$app/stores`; every
  candidate module imports through `$lib`. [V]
- The canvas sets no `minZoom`/`maxZoom`, so xyflow's 0.5 to 2 applies while its
  own buttons use `CANVAS_MIN_ZOOM` 0.1 to 4; the loop editor passes both. [V]
- `BlueprintCanvas.svelte` passes `deleteKey={null}` so xyflow's delete pipeline
  never runs; the guard lives in the file that stays in Polaris. [V]
- Hard-coded control sizes in shared chrome: `CapsuleGear` 30px; viewport
  controls `--dc-control-h-sm` (2rem). Inline rename is explained only by a
  "Click to rename" tooltip. [V]
- xyflow overrides live inside the Merak monolith; the site copies the generic
  ones into its `canvas.css` (plan WD3, section 5.4). Polaris hides the dot grid at night; the site would need it. [V]
- Polaris has no root-canvas keyboard connection; only the loop editor's `c`
  chord exists. xyflow itself has no keyboard edge creation. [V]
- Licensing: Locust is Apache-2.0; dreamcolor10 has no LICENSE file and only
  `license = "MIT"` in Cargo metadata; all relevant commits are the owner's;
  Phosphor icons are MIT but no notice ships with them. The owner confirmed on
  2026-10-04 that they own the code and licensing needs no step; the fork
  copies no Polaris icons. [V; owner]
- `@33ccff/galaxy` is a repository-map visualization, not a design system; no
  canvas file imports it. It was the precedent for a Git dependency pinned to a
  full SHA, considered for a shared package before the owner chose a fork. [V]
- Which shared pieces the web editor would use: `panelCamera.svelte.ts` glides
  the camera when a slide-over panel opens (the site's settings use a page
  column); `CanvasFrameBar.svelte` is a breadcrumb for nested frames (the site
  has none); `CanvasDock*` and `dnd.ts` serve an icon dock (the site's + Add is
  a list); `backEdgeGeometry.ts` routes loop edges (the site refuses cycles).
  None of these is forked. [V source headers; I]

### 1.4 Polaris authoring patterns

Worth keeping [V source; I mapping]: a start surface that keeps the editor
usable (a start row outside the map on the site); starters inserted as one undo
step; quick setup with a footer counting hidden configured values; sectioned
click-to-add (the site's + Add list, not a dock); question-phrased settings
fields with radio rows and descriptions (a page column on the site, not a
slide-over); the start-card reference highlight ("No step reads this input
yet"); honest meaning rows for edges; issues rendered at their control with
one-click recovery that re-checks before applying; change receipts; a
publication preview with consequence copy; one primary action; one gesture per
undo entry.

To avoid [V, several from Polaris's own UX audit scoring the editor 13/20]:
monolith and guard-flag soup; graph-first for non-graph arrangements; four
doors on the empty state; essential text only in tooltips; copy matched to
backend prose with regexes; raw IDs; free-text references; keyboard gaps and
focus lost to `<body>`; announcement noise; validation only after saving;
browser chord collisions (⌘S, ⌘D); mouse-only dock drag; hard-coded colours;
IDE density; over-claiming.

Not to copy: the Merak catalog, node kinds and palette, run controls and
budgets, `productApi` and native data flow, the `polaris.blueprint` envelope,
and the draft subscription and presentation compare-and-swap, which need a
live daemon and belong to O10. Polaris's undo deliberately excludes layout for
multiple writers; a single local author expects layout in undo. [V/I]

### 1.5 The site

- Every route is prerendered; CSS is inlined per route; CSS loaded through a
  dynamic import is not inlined. In 1.6.0, xyflow's `style.css` is 18,920 bytes
  and its `base.css` 13,903, while the built `/start` page inlines one style
  block of 7,503 characters. [V]
- `ssr = false` on a prerendered route yields an empty shell. [V]
- Tests are `node --test`; Svelte components are read as text. Node's
  strip-only mode refuses non-erasable TypeScript such as `enum`
  (`ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX`). [V]
- `CopyPrompt.svelte` renders into a `<p>` with `white-space: normal` and uses
  the whole prompt as the button's description; `copyText` never claims success
  early. [V]
- The header links are pinned by a `deepEqual` test, and `aria-current` needs an
  exact path. [V]
- CI runs no site checks. The site sets no content security policy; SvelteKit 3
  still offers `kit.csp`, emitted as a `<meta>` tag on prerendered pages. [V]
- The site's fonts are subset to Latin: no arrows other than U+2191 and U+2193,
  no geometric shapes (U+25xx), no U+2261. Symbol glyphs would render in
  fallback fonts and be read as "white square" and the like. [V source; I
  rendering]
- Token contrast: `--color-border` (`#2b2c28`) is 1.44:1 on the background and
  1.15:1 on `--neutral-900`; `--color-text-faint` (`#6f6f68`) is 3.99:1 and
  3.20:1. WCAG 1.4.11 asks 3:1 for graphics that carry meaning. [V computed]
- The homepage says "Early development: nothing to install yet"; `/start` says
  Polaris "is not available yet"; first contact forbids links to a connector or
  download that does not exist; the lane C log says the site names Polaris only
  as the complete offering and links nowhere. [V]
- The site uses "published" only for Locust's setup; it defines neither "goal"
  nor publishing a blueprint, both of which the agent will use. [V]
- Neither plan proposes an editor on the site; the documentation plan allows "an
  explicit reviewed site component using checked data". [V]

### 1.6 The agent and the daemon

- The CLI prints `{"ok":…}` envelopes with `--json`; `locust call OPERATION`
  reaches any registry operation; the bound launcher refuses identity flags;
  MCP refuses the owner credential and lists no tools when the daemon is down,
  so offline validation must use the CLI. [V]
- `doctor` is a CLI subcommand, not a registry operation, so it has no MCP tool
  name. [V]
- `goal.create` needs the daemon-wide `manage_goals` grant, which onboarding
  grants as false; an onboarded agent gets `denied`. The owner's direct act
  stands in for the grant, and the creating principal becomes the goal's owner
  and coordinator. [V]
- **Owner authority is reachable from a shell.** The bound launcher refuses
  `--owner`, but it execs the raw binary, whose `--owner` reads
  `owner.credential` from the state directory owned by the same OS user, and the
  CLI's own error for a missing credential suggests "use --owner for owner
  authority". Only the launcher and MCP refuse owner authority; the prompt must
  forbid it. [V]
- Idempotency keys are caller-chosen, 32 hexadecimal characters, retried only
  with identical arguments and never reused for another operation. [V]
- CLI dispatch builds `group.leaf`, so a three-level `blueprint draft create`
  needs a change or a flattened name. [V]
- O7 promises offline contract discovery, validate, explain, normalize and diff;
  revisioned draft and publish; structured diagnostics; and a refused contract
  mismatch with no negotiation. [V plan]
- "Create the task through the daemon" can mean: create the blueprint (draft
  and publish); start a goal with it; use it for a task inside a goal (needs
  delegated variations, M2, and only within the goal's delegated choices); or
  put the JSON into a task's text, which creates no definition and leaks it
  into a shared goal. Only the first is safe as a default. [I]
- Binding a published definition to a goal makes it and its approved inputs
  available to the goal's members; first contact requires sharing to be its own
  approval naming the recipient and the material. [V]
- The binding constraint on size is the model's context and transcription
  fidelity, not the wire (64 MiB frames). A file-based CLI path avoids
  retyping; an MCP-only agent must re-emit the JSON, and no agent can be trusted
  to compute SHA-256 itself, so a Locust-reported source length and digest let
  it prove fidelity. [V limits; I consequence]

### 1.7 Ecosystem

- **xyflow.** 1.6.0 passes node `ariaLabel` to the DOM; 1.6.3 makes touch
  panning win over selection; 1.7.0 fixes resize ordering. Svelte Flow 2 (in
  `next`) changes hooks to accessors, replaces `colorMode` with CSS theming and
  moves styles into a cascade layer. Vite 8 compatibility was confirmed by the
  maintainer. `ariaLabelConfig` exists; its keyboard description keys are
  swapped. No keyboard way to create an edge. A minimal app costs about 62 KB
  gzip over Svelte; CSS 2.75 KB gzip. [V; sizes V measured]
- **xyflow accessibility defaults**, in 1.6.0 and 1.7.0 [V]: the delete key is
  `Backspace`, bound on the window, and deletes every selected node and edge
  unless the event target is an input, select or textarea; every node and edge
  has `tabindex="0"` and the edge layer precedes the node layer in the DOM;
  nodes carry `aria-roledescription="node"` (overridable through
  `domAttributes`) and edges default to "Edge from <id> to <id>"; an
  `aria-live="assertive"` region announces "Moved selected node … New position,
  x: …, y: …" on arrow moves; the wrapper has `role="application"`; `minZoom`
  defaults to 0.5; `preventScrolling`, `zoomOnScroll`, `zoomOnPinch` and
  `panOnDrag` default to true; handles are `role="button"` named "Handle".
  `edgesFocusable`, `nodesFocusable`, `deleteKey`, `minZoom` and
  `ariaLabelConfig` are props.
- **Layout.** `dagre` 0.8.5 is unmaintained since 2019; `@dagrejs/dagre` 3.1.1
  is maintained, MIT, 16.1 KB gzip; `elkjs` is EPL-2.0 or GPL and 443 KB gzip. [V]
- **SvelteKit 3.0.0**, 2026-10-01: Node 22.17, TypeScript 6, Vite 8, `#lib`
  instead of `$lib`, `$app/stores` removed. [V]
- **Pasting.** Claude Code collapses long pastes to a placeholder and is
  reported to truncate above about 50,000 characters (issue #92118) [R]; its
  deep link caps at 5,000 characters [V]. Codex collapses above 1,000
  characters and accepts 1,048,576 [V]. Gemini CLI collapses above 500
  characters [V]. Cursor deep links cap at 10,000 and need confirmation [V].
  Whether terminal agents re-wrap or indent printed blocks so that a copy out of
  the terminal changes bytes is untested; it is likely enough that paste-back
  must not depend on exact digests. [I]
- **Sizes.** Synthetic blueprints: 4.45 KB minified and 7 KB pretty for 12 roles
  and 12 stages; 14.6 and 23 KB for 40 roles. Deflate-raw plus base64url: 943
  characters for 4.45 KB, 2,130 for 14.6 KB. [V measured, synthetic]
- **JSON in JavaScript.** `JSON.parse` followed by `JSON.stringify` puts
  integer-like keys first in ascending order, rounds 9007199254740993 to
  9007199254740992, writes `1.0` as `1`, unescapes `\/` and `é`, and drops
  a final newline. `JSON.stringify` leaves U+2028, U+2029, U+0085, U+202E,
  U+200B and tag characters such as U+E0049 unescaped. Python's `splitlines`
  breaks lines at U+2028 and U+0085, and Unicode line breaking makes U+2028 a
  mandatory break, so a string can forge a line that starts with a sentinel. [V]
  I-JSON ([RFC 7493](https://www.rfc-editor.org/rfc/rfc7493)) limits integers to
  ±(2^53−1) for interoperability. [V source]
- **Delimiting.** Anthropic recommends tags; OWASP recommends clear separation.
  [Spotlighting](https://arxiv.org/abs/2403.14720) found that delimiters alone
  roughly halve attack success, and recommends against relying on them because
  more effective methods exist and an adversary who knows the delimiters can
  subvert them; datamarking cut success from about 50% to below 3% in its
  experiments. [V] Datamarking and encoding change the bytes or defeat review,
  so they conflict with byte-exact, reviewable data blocks; the plan relies on
  Locust's digest check and separate approvals instead, and treats its public
  sentinels as a weak signal. [I] A digest is integrity, not security; base64
  payloads defeat review. [V sources; I design]
- **Share links.** `CompressionStream('deflate-raw')` is in all current engines;
  fragments are never sent to servers but are visible to scripts, extensions
  and history; chat tools truncate long links; deflate can expand by about
  1000:1, so decompressed size needs a limit. [V; ratio I]
- **Browser storage.** Safari's tracking prevention deletes all of a site's
  script-writable storage "after seven days of Safari use without user
  interaction on the site" ([WebKit](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/)). [V]
- **Precedents.** Mermaid Live (versioned `#pako:` fragment, autosave), n8n
  (JSON copy with a sanitize warning), Excalidraw (key in the fragment),
  Mintlify (copy page for an agent), Cursor and Claude Code deep links (prefill,
  capped). What made them friendly: one primary button, an exact preview with
  its size, work never lost, paste-back, no account. [V/I]

## 2. Concepts compared

- **Guided ("Three questions, one picture").** Opens on a valid Open
  collaboration blueprint; three pre-answered questions change it through
  recipes from preset fixtures; only stages connect; role references are chips;
  an always-visible plain-words summary; canvas as progressive enhancement.
- **Canvas-native ("The Agreement Map").** Every arrow is a reference in the
  definition, derived and never stored; meaning chosen before a connection
  exists; completion and selection as separate child nodes; defaults as chips;
  a concept adapter with drift tests; an Outline as a full second projection.
- **Round-trip ("Relay").** The editor as one turn in a loop: the agent's reply,
  with Locust's outputs, is pasted back and mapped onto the canvas; status is
  derived only from pasted evidence; draft memory; an intent ladder including
  Check it; sealed "Kept as is" parts; scoped share links.

Scores from 1 to 5 [I]:

| Concept | First-time friendliness | Contract fidelity | Accessibility and mobile | Implementability on the Polaris canvas pieces | Hand-off robustness | Total |
| --- | --- | --- | --- | --- | --- | --- |
| Guided | 5 | 3 | 5 | 4 | 3 | 20 |
| Canvas-native | 3 | 5 | 4 | 2 | 4 | 18 |
| Round-trip | 3 | 4 | 4 | 3 | 5 | 19 |

- Guided loses fidelity points for a site-written summary (a second describer),
  a handwritten answer classifier, and an editor summary placed in the prompt
  outside the data block, where user-written names become prose. Its hand-off
  is one-way, with no Check-only intent.
- Canvas-native has the strongest fidelity rules (I1 to I7, derived edges, a
  single adapter) but the most to learn and to build: thirteen node types,
  child nodes and a grammar table.
- Round-trip has the most robust hand-off but asks people to paste agent
  replies and parses agent prose, which is fragile; its status claims rest on
  pasted text.

**Choice:** guided as the base, because first-time friendliness is the owner's
first criterion and it satisfies "no graph for simple arrangements" by design.
Grafted from canvas-native: derived edges, the adapter and drift tests,
byte-preserving patches with invariants, meaning-before-existence connections,
defaults as chips, "picked for you", the full list view and the job gesture.
Grafted from round-trip: the Check-only intent, one block format for every
exchange, sealed parts, paste-back with a change receipt, scoped share links,
and grouping problems by what the person must do. The weak points of the base
are removed by running Locust's own explainer in the page (plan WD4), by taking
card sentences from its fragments, and by keeping all user text inside the data
blocks (plan WD7).

## 3. Review of the draft plan

Four reviews produced 49 findings: 12 on first-time, phone and
assistive-technology use, 12 on engineering, 14 on contract fidelity and 11 on
trust and safety; none was a blocker. The review covered the shared-package
version of the plan; after the owner chose a fork, the package-specific
changes below lapsed, and the plan's `CanvasSurface` became the site's
`OrgCanvas`. Each finding's evidence was checked
against source or by a probe before it was applied. The table groups them by
theme; plan identifiers are those of the final plan.

| Theme | Verified evidence | Change in the plan |
| --- | --- | --- |
| xyflow keyboard and screen-reader defaults | Section 1.7's accessibility defaults; Polaris's `deleteKey={null}` stays behind in `BlueprintCanvas.svelte` | `OrgCanvas` fixes `deleteKey={null}`; card-scoped delete; lines out of the Tab order; card order from the List; skip links; full card labels; coordinates silenced; zoom floor set by `OrgCanvas` (WD3, section 4.12) |
| Contrast, glyphs and sizes | Section 1.5's font subset and contrast; section 1.3's control sizes and tooltip | Kind words and SVG icons; meaning-bearing outlines at 3:1; control and handle tokens; a Name field; refused-drop reasons at the drop point (sections 4.4, 4.12, 4.13) |
| Phone use | xyflow's scroll and pan defaults; the Describe door lived only on the map's start card | A start row outside the map; a phone column order; a read-only preview; Send to my computer; the sticky bar hidden on focus; real-device checks (J7, W8) |
| Vocabulary | Section 1.5: the site defines no "goal" or blueprint publishing; the draft used "contract X", "Locust ID" and "definition hash" for related things | A fixed vocabulary line; a glossary; "blueprint format"; "definition hash" on page and in the prompt (sections 4.6, 4.11) |
| Lost work | One storage key per marker; undo in memory; Safari's seven-day deletion (section 1.7) | A list of local blueprints that is never overwritten, with defined record contents and Delete (WD10, section 4.9) |
| Second describer | Card rows, meaning rows, labels and digest were site sentences; the D14 line sat inside "Locust's explanation" | Explainer fragments (WR8); a four-part panel separating page text (section 4.6) |
| Site-chosen authority | Deleting a role reset rules to "any member"; repairs and "picked for you" had no stated source; D4 and D12 need named authorities | Deletes leave selectors unset; values only from fixtures and the catalog; broadening repairs never one click (I5, I6, section 4.7) |
| Sealed parts | Rename and delete rewriting could not avoid sealed subtrees | A whole-tree reference index; edits touching sealed parts refused (WR1, WR6, section 4.4) |
| Byte preservation | Section 1.7's JSON probe | An editor text form; I4 and I7 restated; WR7 limits keys and integers (WD7, WD13) |
| Forged boundaries and hidden text | Section 1.7's separator and hidden-character probe | Escaping in the editor text form; Check before copying; WR9; hostile-fixture tests (section 6.3, WV25) |
| WebAssembly build and delivery | Section 1.2's getrandom and reproducibility findings; the fallback would have made site builds need Rust | Two remedies tried in W0; pinned tools and remapped paths; a committed module with behaviour checks if bytes differ (WD4, WD5) |
| WD4 fallback | The Ajv alternative left hash, diff, codes and parity unspecified | A table of what each feature becomes without WD4 (plan section 3) |
| Package access and supply chain | The review draft's package needed repository access and a CI token (lapsed); npm runs a Git dependency's `prepare` on install (method spot checks); no content security policy (section 1.5) | `kit.csp` and integrity on the WebAssembly fetch (WD14). The package items lapsed when the owner chose a fork |
| Paste-back | Agents' replies must carry exact digests; copy-out was untested | Lenient inbound extraction with the real loader; a saved-file route; W0 measures copy-out (section 6.7) |
| Copy check | The draft relied on the model computing SHA-256 and had no MCP-only branch | Locust reports length and digest of what it read (WR10); an MCP-only path; `doctor` as a fixed CLI phrase (section 6.2) |
| Owner authority | Section 1.6's raw-binary path | Step 1 forbids other Locust programs, owner options and the state directory; W11 checks it |
| Goal and task approval | The goal step hid the administrator, sharing with members and the daemon-wide grant; the task intent could prompt a goal change | A separate, informed yes; changing an existing goal is out of scope; the task intent waits for WR12 |
| Draft replacement | "Update it at its current revision" replaced without a diff; no key chosen before a write | Diff against a shown revision, explicit yes, new idempotency key per write |
| Identities in links | Section 1.1's identity selector; the draft claimed links held "no people" | Disclosure in Copy link; Check before copying; the goal step names them |
| Format mismatch | The draft offered an editor that does not exist and suggested updating Locust | Format line next to copy; development labelling; the agent names the person's contract and never suggests updating (WD6, section 6.5) |
| Exclusive-sounding words | "Picked up" throughout, against D5 | "Who may work on what?"; a banned-word test (WV26) |
| Composition test | Rows with free answers could not hash equal to one preset | Split assertions (section 4.5, WV04) |
| Ownership and order | Workstream rules; W4's exit needed W5 and W6; `check_docs.py` fails on `main` | Owner column; corrected exits (section 7). The adoption ordering lapsed with the fork |
| Polaris naming | J4's refusal copy named Polaris | Neutral copy from the loader's diagnostic; a visible-copy test (WV26) |
| Research accuracy | The draft overstated Spotlighting's result for delimiting | Corrected in section 1.7; the risk row now ranks Locust's checks first |

Parts of findings not adopted, with the verified reason:

- Defaulting to Check it when WD4 fails, and for blueprints opened from a link:
  in every intent the agent runs Locust's validator before saving, and the only
  pre-authorized write is a private draft that may be incomplete (main plan
  section 4.1); the "From a link" marker and Check before copying cover link
  risk, and a Check-it default would penalize the phone-to-computer route.
- Refusing raw U+2028, U+2029 and U+0085 in pasted blocks: JSON permits them
  inside strings, and inbound pastes are read leniently; the extractor splits on
  LF only and re-serializes in the editor text form, which escapes them before
  anything shows or copies them.
- Telling a person with a different format that they can update Locust: under
  D3 that can make Locust refuse their existing goals.
- "Blueprint ID" for the definition hash: the agent and Locust say "definition
  hash", so the page uses the same term and explains it.
- A test that editor sources contain no "Polaris" string: no product's format is
  recognised by name, and the test covers visible copy, which is what the site's
  rule concerns.
- Recording requests in the lane C log only at W9: they must be answered before
  W1, so W0 records them and W9 updates them.
- Saying "nothing starts until you start a goal": under D14 starting a goal
  starts no agent either, so the line says publishing "shares nothing and starts
  nothing".

## 4. Rejected and deferred alternatives

| Alternative | Disposition and reason |
| --- | --- |
| Import `BlueprintCanvas.svelte` wholesale | Rejected: Merak node kinds, run state, loop and depth geometry; main plan section 8 forbids reusing them |
| A shared canvas package in its own repository, pinned by SHA in both apps | Rejected by the owner, 2026-10-04: fork from Polaris for the web instead; the two may look different. The review draft recommended it; its install, visibility and adoption details lapsed |
| Package as a subdirectory of dreamcolor10 or Locust | Rejected: npm cannot install a Git subdirectory |
| Fork the generic modules into the site | Adopted (owner decision): the site reformats and restyles them, and divergence from Polaris is accepted |
| Fork every module Polaris shares between its canvases | Rejected: the slide-over camera, dock, frame bar, haptics and inline rename have no use on the web; unused code is dead code |
| Add a `$lib` alias to the site | Rejected: breaks the site's `#lib` convention |
| `ssr = false` for the editor route | Rejected: prerender yields an empty shell; no-JavaScript reading fails |
| Ajv checks plus a site-written explanation | Rejected: a second describer of the contract; Ajv survives only as the WD4 alternative |
| Ajv and WebAssembly together | Rejected: two structural checkers with different messages break "same diagnostics everywhere" |
| Site-written card and line sentences | Rejected: a second describer; sentences come from explainer fragments |
| Computing the definition hash in TypeScript | Rejected: reimplements the canonical codec |
| Building the WebAssembly module in the site's CI with nothing committed | Rejected: every site build would need Rust, against the agent guide and the documentation plan |
| Pasting Locust's outputs back for status (round-trip "Relay") | Deferred: with WD4 the page already has Locust's results; pasted outputs are display-only and agents paraphrase. Revisit if WD4 is rejected |
| Page-side draft memory (draft id and revision) | Replaced: the prompt has the agent look for a same-named draft recorded as made with this editor, show Locust's diff and ask |
| Replacing a same-named draft at its current revision | Rejected: silent last-writer-wins, against the accepted direction's revision checks and reviewable diffs |
| One autosave record per contract marker | Replaced: a list of local blueprints, so links, imports and Start over never overwrite work |
| Resetting a deleted role's rules to "any member" | Rejected: widens authority and conflicts with D4 and D12 |
| Letting the agent apply "meaning-preserving" fixes | Rejected: the agent changes nothing; changes come back as a new block for the editor |
| Agents computing SHA-256 themselves | Rejected: models cannot be trusted to; Locust reports length and digest of what it read |
| Exact digests on blocks pasted back from an agent | Rejected: terminal copies may change whitespace; the loader checks content instead |
| Editor summary inside the prompt | Rejected: puts user text outside the data blocks; redundant with WD4 |
| Completion and selection as child nodes | Replaced by rows plus separate Decision cards: fewer node types, same distinction |
| Decorative edges, frames, per-card colours | Rejected or deferred: arrowheads must mean rules; frames add little |
| Unicode symbol glyphs for kinds and status | Rejected: outside the site's font subset and read aloud as shape names; inline SVG with text instead |
| Recognising another tool's workflow envelope by name | Rejected: names a product the page must not name; the loader's missing-format diagnostic suffices |
| Datamarking or encoding the data blocks | Rejected: changes bytes or defeats review; Locust's digest check and separate approvals protect instead |
| Suggesting a Locust update on a format mismatch | Rejected: a release that changes the contract refuses earlier goals (D3) |
| Deep links into Claude Code or Cursor | Rejected: 5,000 and 10,000 character caps and documented abuse |
| Fetching the blueprint by URL | Rejected: needs server storage; web fetch tools summarize and refuse localhost |
| Base64 payloads; XML tags | Rejected: base64 cannot be reviewed; sentinel lines are harness-neutral, line-testable and avoid characters the site's prompt tests ban |
| vitest, jsdom and testing-library in the site | Rejected: forked pure-module tests run under `node --test`; Playwright covers component and browser behaviour |
| `elkjs`; `dagre` 0.8.5 | Rejected: licence and size; unmaintained |
| Versioned editor routes | Deferred to the O12 continuity decision |
| Literal `task.propose` with the JSON as text | Rejected: creates no definition and leaks the JSON into a shared goal |
| Localhost bridge or browser extension | Rejected: authoring credentials in untrusted browser channels (D8, O10.7) |

## 5. Open questions

1. Will O1 accept the export requests WR3 to WR9: the diagnostics catalog with
   repairs and defaults, the presentation type and element IDs, erasable
   TypeScript, reference annotations, key and integer limits, explainer
   fragments and a hidden-character diagnostic?
2. Are role and stage names identifiers or presentation labels? A rename's
   effect on the definition hash depends on it.
3. Do optional tasks in Open collaboration get a default completion rule?
4. Will offline `validate`/`explain` report the definition hash, and will every
   source-reading operation report the length and SHA-256 of what it read
   (WR2, WR10)?
5. Does `draft.create` accept presentation in the same call, or only through a
   separate presentation update?
6. Which grants do enrolled agents need for draft and publish? Goal creation
   needs `manage_goals` today.
7. Is publishing a definition whose capabilities the daemon lacks refused, or
   allowed and only blocked at goal creation?
8. What fields does the goal-creation successor take for the definition hash,
   inputs, role bindings and administration identity, and which operation
   creates a task from a published definition (WR12)?
9. Are authored deadlines expressible under the timing exclusions?
10. Which getrandom remedy does lane A accept for the WebAssembly crate: a
    target-specific `wasm_js` feature, or a crate or feature boundary around the
    pure contract?
11. Whether a binary export may live in Git (WD5).
12. Will the availability record carry capability and blueprint-format entries
    (WR13)?
13. Which style policy does xyflow's inline styling need under the site's
    content security policy?
14. Should the header link appear before a Locust build with blueprint support
    is published, given the site is not deployed?
15. Is the Polaris repository `merak10` or `dreamcolor10`? The main plan and the
    checkout disagree.

## Sources

Local sources, read 2026-10-04:

- Locust at `6b9365f`: [accepted direction](../docs/organization-blueprints.md),
  [blueprint implementation plan](../docs/organization-blueprints-implementation-plan.md),
  [public documentation plan](../docs/public-documentation-plan.md),
  [first contact](../docs/first-contact.md), [workstreams](../docs/workstreams.md),
  the [lane C log](../docs/lane-c-log.md), the site under
  [sites/locust.farm](../sites/locust.farm/README.md) including its
  [fonts](../sites/locust.farm/src/lib/styles/fonts.css) and
  [tokens](../sites/locust.farm/src/lib/styles/tokens.css), the
  [root Cargo manifest](../Cargo.toml), the
  [contract crate manifest](../crates/locust-proto/Cargo.toml), the
  [CLI arguments](../crates/locust/src/cli/args.rs),
  [CLI connection](../crates/locust/src/cli/connection.rs),
  [MCP](../crates/locust/src/mcp.rs),
  [launcher](../crates/locust/src/installation/setup/launcher.rs),
  [access rules](../crates/locust-core/src/node/access.rs),
  [goal requests](../crates/locust-core/src/node/requests/goals.rs),
  [onboarding](../crates/locust/src/installation/onboarding.rs) and the
  [operating skill](../skills/locust/SKILL.md).
- Polaris at `01d8aa3c4` (not in this repository): `crates/polaris/frontend/`
  `package.json`, `scripts/check-galaxy-dependency.mjs`,
  `src/lib/components/blueprint/` (including `BlueprintCanvas.svelte`,
  `canvasConstants.ts`, `nodes/CapsuleGear.svelte` and
  `nodes/NodeNameEditor.svelte`), `src/lib/styles/design-tokens.css`,
  `src/lib/blueprints/`, `src/lib/workflow/layout.ts`,
  `docs/BLUEPRINT_V1_UX_AUDIT_2026-09-16.md`, the installed
  `@xyflow/svelte` 1.6.0 and `@xyflow/system`, and `.github/actions/galaxy-access/`.
- getrandom 0.4.3 `src/backends.rs` in the local Cargo registry.

External sources, accessed 2026-10-04:

- npm registry entries for [@xyflow/svelte](https://www.npmjs.com/package/@xyflow/svelte)
  and [@dagrejs/dagre](https://www.npmjs.com/package/@dagrejs/dagre);
  `@xyflow/svelte` 1.7.0 component sources on [unpkg](https://unpkg.com/@xyflow/svelte@1.7.0/).
- [npm install documentation](https://docs.npmjs.com/cli/commands/npm-install).
- [Svelte Flow server-side rendering guide](https://svelteflow.dev/learn/advanced/server-side-rendering).
- [Ajv standalone validation code](https://ajv.js.org/standalone.html).
- [Spotlighting: defending against indirect prompt injection](https://arxiv.org/abs/2403.14720),
  HTML version 1.
- [MDN: CompressionStream](https://developer.mozilla.org/en-US/docs/Web/API/CompressionStream).
- [WebKit: full third-party cookie blocking and more](https://webkit.org/blog/10218/full-third-party-cookie-blocking-and-more/).
- [RFC 7493, the I-JSON message format](https://www.rfc-editor.org/rfc/rfc7493).
- [WCAG 2.2](https://www.w3.org/TR/WCAG22/), success criteria 1.4.11 and 2.4.11.
- [Unicode line breaking algorithm (UAX #14)](https://www.unicode.org/reports/tr14/).
- xyflow issues #5732 and #6038 and pull request #5866; Claude Code issue
  #92118; Codex issue #13040; Mermaid Live, Excalidraw and Cursor deep-link
  documentation, as reported by the ecosystem reading.
