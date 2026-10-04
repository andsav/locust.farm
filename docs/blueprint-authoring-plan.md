# Blueprint editor on locust.farm: implementation plan

Date: 2026-10-04. **Status: implemented in the site at `/blueprints`; checked by
unit tests, conformance vectors and a browser walkthrough. Not yet tried with
real agents or first-time users (section 8).** Baseline: Locust `74ec9fb`, after the
organization implementation landed (`809bbbe`) and the contract rename. The
Polaris checkout read for the canvas fork is `dreamcolor10` at `01d8aa3c4`.

Related documents: the [accepted direction](organization-blueprints.md), the
[blueprint implementation plan](organization-blueprints-implementation-plan.md),
the [semantics](organization-blueprints-semantics.md), the
[public documentation plan](public-documentation-plan.md) and the
[research note](../research/blueprint-authoring.md), which records sources,
measurements, the compared designs and the reviews behind this plan.

## Owner decisions, 2026-10-04

1. The editor lives on the marketing site at `/blueprints`. A person who has
   Locust builds a blueprint there and copies one prompt that has their coding
   agent add it to their Locust.
2. The editor must be easy for a first-time user. Wording is plain and short,
   with no clever headings.
3. The page opens with the question "How does your team work?" and six ways of
   working, each with a small animated diagram. Everything else adjusts the
   chosen way.
4. The canvas shows only stages and the order between them. Roles, work rules,
   decisions, task types and advice are properties, edited in panels.
5. Nodes and the side panel keep the look of Polaris's nodes and settings panel,
   in Locust's colours, font and tone.
6. The canvas code is forked once from Polaris into the site. There is no
   shared repository or package. The two may diverge. Licensing needs no step.
7. All checks that need no goal or machine context run in TypeScript in the
   page. There is no WebAssembly build. Locust stays authoritative: the agent
   re-checks with Locust before anything is saved.
8. The contract uses plain names: `task_types` (was `variations`), a stage's
   `task_type`, `decisions.finish` (was `closure`) and a stage's `runner` (was
   `materializer`). Implemented in `74ec9fb`.

Decision 7 departs from the blueprint plan's O1.4 ("prevent handwritten copies
in Polaris or the site") and from the accepted direction's "use the same
validator". The test cases in section 5 keep the copy honest.

## 1. Outcome and scope

A person opens `locust.farm/blueprints`, picks how their team works, adjusts a
few plain choices, optionally lays out stages on a map, and presses one button
to copy a prompt. Pasted into their agent, the prompt makes the agent check
the blueprint with their Locust, explain it in Locust's words, save it as a
private draft and ask before publishing it.

Done means:

- Each of the six ways of working can be chosen and copied without any edit.
- Every part of the contract the editor can show is editable by keyboard and
  on a phone, without the canvas.
- The page's checks agree with Locust on every test case (section 5).
- Layout changes never change the blueprint itself.
- A blueprint written by an agent opens in the editor; parts the editor does
  not show are kept unchanged and listed.
- The copied prompt validates and saves with the real CLI in an automated test.
- The site's lint, type check, unit tests, browser tests and build pass.

Not in scope: any connection from the browser to Locust; accounts or server
storage; starting a goal from the prompt (section 7.4); changes to Polaris; a
light theme.

## 2. What Locust provides

Verified at `74ec9fb`.

| Area | Fact | Used for |
| --- | --- | --- |
| Contract | `schema_version` 1; top-level `roles`, `work`, `decisions`, `task_types`, `flow`, `context` ([types](../crates/locust-proto/src/organization.rs)) | The editor's document model |
| Exports | [contract](reference/generated/organization.contract.json) (schema, presets, operations), [schema](reference/generated/organization.schema.json), [runtime contract](reference/generated/runtime.contract.json), written and drift-checked by [check_blueprints.py](../scripts/check_blueprints.py) | Presets, schema tests, operation names in the prompt |
| Presets | `open`, `coordinator`, `peer-review`, `review-panel`, `independent-attempts`, `pipeline` ([examples](../examples/blueprints/open.json)) | The six ways of working |
| Offline checks | `locust blueprint validate`, `explain`, `normalize`, `diff`, `contract`, `schema`, `examples` ([CLI](../crates/locust/src/cli/blueprint.rs)); diagnostics have `code`, `severity`, `phase`, `path`, `message`, `correction`, `related_paths` | Prompt steps; the test cases |
| Validation | [loader](../crates/locust-core/src/organization/strict_json.rs), [rules](../crates/locust-core/src/organization/validation.rs), [explanation](../crates/locust-core/src/organization/explanation.rs), [normalization](../crates/locust-core/src/organization/normalize.rs), [diff](../crates/locust-core/src/organization/diff.rs) | Ported to TypeScript (section 5) |
| Catalog | Private drafts, a separate presentation record holding opaque `data_json` that never affects identity, and immutable publications; operations `blueprint.draft.create`, `blueprint.draft.update`, `blueprint.drafts`, `blueprint.presentation.update`, `blueprint.publish` and others ([API](../crates/locust-proto/src/api.rs)) | Prompt steps; the stage layout |
| Flow | A stage is ready when its `requires` evidence exists; the stage's runner's Locust then creates its task and delivers it to the stage's recipients. No agent is started ([semantics](organization-blueprints-semantics.md)) | The map's wording |
| Goal creation | `goal.create` takes inline `blueprint_json`, `roles` and `inputs`, needs the daemon-wide `manage_goals` grant, makes the creator the only member, and needs required inputs as blobs that exist only after the goal does | Why the prompt does not start goals |
| Site | SvelteKit 3, Svelte 5, Vite 8, `node --test`, prerendered pages, a docs site under `/docs`, CI in [site.yml](../.github/workflows/site.yml) | Where the editor lives |

Known Locust quirks the port does not copy: the decoder accepts a root array
and some shapes the schema does not allow. The test cases stay inside the
schema's shapes, and the research note lists the quirks for lane A.

## 3. The page

### 3.1 Layout

The page shows, tells little. Under the site header and the headline it has the
six ways of working as a strip of animated pictures, then one framed editor
that fills the rest of the window:

- **Toolbar.** The blueprint's name, a save mark, icon buttons (undo, redo,
  open, saved, copy link, download, start over; problems, in words, see the
  prompt, manual, set up) and the copy controls.
- **Rules rail** on the left: roles, how work starts, when a task is done, and
  More.
- **Map** filling the rest: the stages, or, with no stages, a large animated
  picture of the current rules.

Every icon has a tooltip with its name and, where needed, one sentence. Longer
text lives in one side panel that slides over the right of the map, as in
Polaris: stage and role settings, More rules, problems, the blueprint in words,
the prompt, open and saved blueprints. There are no modal dialogs. On phones the
page is one column: ways, toolbar, rules, the map as a preview that opens full
screen, and the stages as a list; the side panel covers the screen.

The design context, shared with the impeccable design skills, is in
[.impeccable.md](../.impeccable.md).

### 3.2 First visit

The page opens with "How does your team work?" and six choices, each a card
with its animated diagram (from the approved draft), a name and one sentence:

| Way of working | Sentence | Preset |
| --- | --- | --- |
| Open | Everyone works freely and shares what they find. You say when your own work is done. | `open` |
| Coordinator | One person hands out work and accepts the results. | `coordinator` |
| Peer review | Anyone can work on a task. Someone else has to approve it before it counts. | `peer-review` |
| Review panel | Several reviewers look at each result. It counts once enough of them approve. | `review-panel` |
| Independent attempts | Several people try the same task in their own way. A judge picks the result to use, and the other attempts are kept. | `independent-attempts` |
| Pipeline | Work moves through steps in order. Each step starts when the one before it is done. | `pipeline` |

Each card shows only its picture and name; the sentence is its tooltip and is
read to screen readers. Open is selected on arrival, so the copy button works
straight away. Choosing a card loads that preset as one undoable change. Once
the rules differ from the chosen way, its card says "changed"; clicking it again
goes back to the way, and undo keeps the changes.

### 3.3 Rules rail

Each question is an icon heading with one word (Roles, Start, Done) and the
question in its tooltip. Answers are rows of icon choices with one word each
and a sentence in each choice's tooltip. Roles are chips; a chip opens the
role's settings and "+" adds one. Details of the chosen answer, such as who
reviews and how many approvals, appear under it as icon-labelled fields. The
less common rules are under More, in the side panel.

| Question | Contract |
| --- | --- |
| Who is involved? Everyone in the goal, or named roles. Roles are filled with people when someone starts a goal. | `roles` |
| Who can suggest tasks? Who can share findings? | `work.propose`, `work.publish` |
| How does work start? Anyone can start working on a task (several people may work on the same one), or someone hands out work and the person asked must accept. | `work.starts` |
| When is a task done? The person who did it says so; someone else reviews it (how many, and whether the author may review); an automated check reports success. | `decisions.completion` |
| Do you need one final answer? If yes, who picks it. | `decisions.selection` |
| Who can say the whole goal is finished? Nobody (the goal stays open), or one named role. | `decisions.finish` |
| Advice for everyone. Shown to agents; never a permission. | `context.guidance` |
| Starting material: text or files supplied when a goal starts. | `context.inputs` |
| Different rules for some tasks. Named sets of rules that tasks or stages can use. | `task_types` |

The first three questions are always visible. The rest sit under "More
choices", which shows how many differ from the chosen way. Who-pickers offer
"everyone in the goal", each role, and where allowed "the person who created
the task" or "the author of the work"; specific people by key are shown only if
an imported blueprint contains them. Choices that Locust cannot do (first
person to grab a task keeps it, approval when nobody objects, majority votes,
private roles) are listed under "Not possible" with Locust's reason and what to
do instead.

### 3.4 Stage map

Nodes are the keys of `flow`. Arrows are each stage's `requires` entries,
drawn from the earlier stage to the later one and labelled in plain words:
"when draft is published", "when draft has a review", "when draft is
complete", "when a result of draft is picked" (evidence `publication`,
`review`, `completion`, `selection`). Nothing else is drawn.

- **Empty map.** A large animated picture of the current rules, chosen by
  `pictureFor` in [presets](../sites/locust.farm/src/lib/blueprint-editor/model/presets.ts):
  handed-out work, then one final answer, then reviews. Changing a rule changes
  the picture.
- **Adding.** The round "+" places a new node and opens its settings. Dragging
  from one node's handle to another opens a small menu: "Start review when
  draft is…" with the four choices. Dropping on empty map adds a stage that
  waits for the first one to be complete and opens it, as in Catalyst; other
  stages keep their places. A connection that would make a loop is refused at
  drop time: "That would make draft wait for itself."
- **Node.** A Polaris capsule: a round accent disc, the stage name, and while
  selected a row of icon chips (work goes to, run by, own rule) with tooltips.
  Problems show as a warning icon and count on the node.
- **Settings panel.** Polaris's settings panel: a header with the node's disc
  and name, one scrolling body, and a footer with Delete and the save state.
  Fields, in order: Name; "Starts when" (the prerequisites as a list, each with
  a stage and one of the four choices, plus "Add"); "Who gets this step's
  work?" (`recipients`); "Run by" with the help "Whose Locust creates this
  step's task and sends it out when it is ready. Must be one person."
  (`runner`); "When is this step done?" with "Same as the rules on the left" or
  its own answer.
- **Stage rules without new words.** Choosing its own answer for "When is this
  step done?" creates or updates a task type named after the stage. The word
  "task type" appears only under "More choices", where all task types are
  listed.
- **Layout.** Positions live in the presentation record, under the page's own
  key: `{"locust.farm": {"stages": {"draft": {"x": 0, "y": 0}}}}`. Other keys
  in that record are kept. Missing positions are filled by an automatic left to
  right layout. Renaming a stage renames its layout key and every reference to
  it in one undoable change.
- **Keyboard and screen readers.** Every stage has a spoken description, and
  on phones every stage is also listed under the map as text. Tab moves between stages, Enter
  opens settings, C starts a connection from the focused stage and asks for
  the target and the choice. Backspace on the page never deletes anything
  unless a stage has focus.

### 3.5 Problems and the blueprint in words

The toolbar's status button is a check when nothing is wrong and an ember
warning with a count otherwise. It opens the problems in the side panel: a plain
sentence first, then "Show me", which selects the field or node, and "Details"
with the code and path. Problems block nothing: copying with problems produces a
prompt that saves an unfinished draft and does not publish.

"In words" opens a plain summary written by the page from the current
blueprint, then "What Locust will say": Locust's own explanation lines,
produced by the TypeScript port, which is what the agent will show.

### 3.6 Copy

The ember "Copy prompt" button is the page's one primary action and is always
in the toolbar. Beside it, two choices for what the agent should do, each with
its sentence in the tooltip:

- **Add** (default). "Your agent adds it to your Locust as a private draft and
  asks you before publishing."
- **Check.** "Your agent checks it with Locust and explains it. Nothing is
  saved."

After copying, the button says "Copied" and a note says "Copied. Paste it into
your agent." The eye button shows the full prompt with its size; a failed copy
opens it with the text selected. The set-up icon links to `/start`.

### 3.7 Keeping work

- The browser keeps a list of saved blueprints (local storage, one record per
  blueprint). Opening a link, importing or starting over creates a new record;
  nothing is replaced silently. "Saved in this browser" lists them with Delete,
  in the side panel. A check beside the name means saved; an ember warning
  means this browser is not keeping it.
- "Copy link" puts the blueprint and its layout in the address after `#`,
  compressed. The part after `#` is not sent to any server. Opening such a link
  creates a new record and shows "Opened from a link. Read its names and advice
  before you copy it."
- "Open a blueprint" accepts pasted JSON, a pasted agent reply containing the
  prompt's blueprint block, or a file.
- "Download" saves the blueprint as JSON.

### 3.8 Wording

Contract words appear only under a problem's "Details" and in the prompt's data.
The page says: "task type" only where named rule sets are listed; "runs" or
"run by" for `runner`; "say the goal is finished" for `finish`; "pick one
final answer" for `selection`; "advice" for `guidance`; "starting material"
for `inputs`. Visible words are kept to names and one-word labels; sentences
are in tooltips and the side panel.

## 4. Look and feel

The fork keeps Polaris's shapes and behaviour and uses the site's tokens:

| Element | From Polaris | On the site |
| --- | --- | --- |
| Stage node | `NodeCapsule.svelte`: 216×46 capsule, round accent disc, title, right slot; selected nodes grow an overlay with chips; hover and selection light the rim | Same shapes; fill `--neutral-900`, rim `--neutral-800`, text `--color-text`, accent disc and selected rim in ember; Martian Mono |
| Settings panel | `InspectorPanel.svelte`: slide-over, header with the 40px accent disc and title, scrolling body, footer band with Delete and save state | Same structure; panel background `--neutral-950` with a hairline `--color-border`; labels in the site's label style |
| Camera | `panelCamera.svelte.ts`: pans so the selected node stays visible beside the panel | Forked as is |
| Map ground | Dot grid, viewport controls | Dot grid in `--neutral-850`; a floating plate of icon buttons for zoom, fit and tidy |
| Icons | Phosphor, regular weight | The same Phosphor icons, copied into `ui/icons.ts`, with a tooltip on every icon control |
| Focus | Polaris's background change | The site's 1px ember outline |

New tokens are added to [tokens](../sites/locust.farm/src/lib/styles/tokens.css)
only when used: `--color-surface`, `--color-grid`, `--text-ui` and two motion
durations.

## 5. Checks in TypeScript

Modules in `sites/locust.farm/src/lib/blueprint-editor/contract/`, plain
TypeScript that runs under `node --test`:

| Module | Ports |
| --- | --- |
| `load.ts` | Strict JSON: syntax errors, duplicate keys, number ranges (`invalid_json`, `duplicate_key`) |
| `decode.ts` | `schema_version` check (`unsupported_version`) and structure: unknown and missing fields, wrong types, unknown kinds (`invalid_structure`) |
| `rules.ts` | Every validation rule in [rules](../crates/locust-core/src/organization/validation.rs), in Locust's order: `invalid_name`, `unknown_role`, `invalid_participant`, `selector_scope`, `empty_selector`, `impossible_completion`, `invalid_threshold`, `impossible_threshold`, `empty_criteria`, `unknown_task_type`, `unknown_stage`, `unavailable_evidence`, `flow_cycle` |
| `normalize.ts` | The effective rules Locust uses for explanation |
| `explain.ts` | Locust's explanation lines |
| `inspect.ts` | The result shape of `locust blueprint validate --json` |

Not ported: the semantic hash and source hash, which need Locust's binary
encoding and BLAKE3, and checks that need a goal or a machine (role bindings,
grants, drafts).

**Agreement with Locust.** A cases file,
`docs/reference/conformance/organization.cases.json`, lists valid and invalid
documents: every preset and at least one case per diagnostic code and message
template. `check_blueprints.py` runs the built CLI on each case and writes
`docs/reference/generated/organization.vectors.json`; its check mode fails on
drift, as for the other exports. A site test runs the port on every case and
requires the same `valid`, and for each diagnostic the same `code`,
`severity`, `phase` and `path`. Rule messages and corrections must match
exactly. For `invalid_json` and `invalid_structure` the page shows its own
plain message, because Locust's comes from its JSON library. Explanation lines
must match exactly. A rule added to Locust without a case fails the export
check, because the script requires every code in `validation.rs` to appear in
some case's result.

## 6. Canvas fork

Forked once from `crates/polaris/frontend/src/lib/` at `01d8aa3c4` into
`sites/locust.farm/src/lib/blueprint-editor/canvas/`. The site README lists
each forked file with its source path and commit.

| Forked | Change |
| --- | --- |
| `components/blueprint/nodes/NodeCapsule.svelte` | Became `StageNode.svelte`: same pill, disc, rims, ports and selected overlay; site tokens; no run states, inline rename or tooltips |
| `components/blueprint/InspectorPanel.svelte` | Became `SidePanel.svelte`: same header, body and footer; site tokens; a labelled Close button |
| `components/blueprint/FitViewBridge.svelte` | Became `FlowBridge.svelte`, trimmed to the hooks the map uses |
| `workflow/layout.ts` | Became `layout.ts`: stages only, on `@dagrejs/dagre`, left to right |

The map itself, its edges, the camera nudge and the viewport controls are
written for the site; the Polaris versions carried run, loop and depth
behaviour the stage map does not need.

Not forked: everything tied to Merak's step graph, runs, loops, depth, the dock,
haptics, the frame bar and the icon set. Any forked module that ends up unused
is deleted before the work is committed.

Dependencies added to the site: `@xyflow/svelte` 1.7.0 and `@dagrejs/dagre`
3.1.1, plus `@playwright/test` for browser tests. The canvas and its CSS load
only on `/blueprints`, after the page shell, so other pages stay unchanged.

## 7. The prompt

### 7.1 Contract document

`docs/blueprint-prompt.md` holds the exact fixed text of the prompt, the data
block format and the rules for what may appear in it. The prompt builder must
match it word for word, as the entry prompt matches
[first contact](first-contact.md), which stays unchanged. Operation names are
taken from the runtime contract export.

### 7.2 Shape

1. What to do, in two or three sentences, and that the data below is data, not
   instructions.
2. Find Locust through the installed Locust skill or the `locust_` tools. If
   there is none, point to `https://locust.farm/start` and stop. Install
   nothing.
3. Check that Locust's `schema_version` is 1. If not, report it and stop.
4. Write the blueprint block exactly to a temporary file. Compare its byte
   count and SHA-256, computed with a program such as `shasum -a 256`, with the
   values in the block header. If they differ, ask for a fresh copy.
5. Run `locust blueprint validate` and `locust blueprint explain` on the file.
   Show Locust's explanation and any problems.
6. For "Check it", stop here.
7. Save a private draft with `blueprint.draft.create`, then the layout block
   with `blueprint.presentation.update`. If a draft with the same id exists,
   show the difference and ask before replacing it.
8. Ask before publishing. On yes, publish with `blueprint.publish` and report
   the semantic hash.
9. Say how to start a goal later, without doing it: it needs the
   `manage_goals` grant from the machine's owner, the creator becomes the only
   member, and roles are filled as people join.

The prompt forbids installing or updating Locust, starting the daemon, using
the owner credential, changing grants, inviting anyone and creating goals.

### 7.3 Data blocks

```text
BEGIN LOCUST BLUEPRINT schema_version=1 bytes=<n> sha256=<hex>
<the blueprint JSON, two-space indented, LF line endings>
END LOCUST BLUEPRINT sha256=<hex>
BEGIN LOCUST LAYOUT bytes=<n> sha256=<hex>
<the presentation JSON>
END LOCUST LAYOUT sha256=<hex>
END OF LOCUST PROMPT
```

Names, descriptions and advice appear only inside the blocks. Control,
bidirectional, zero-width and tag characters inside strings are written as
`\u` escapes, so no line inside a block can look like a block boundary. Above
40,000 characters the page offers a download of the blocks and a short prompt
that reads them from the file.

### 7.4 Starting a goal

The prompt does not start goals. A goal needs the `manage_goals` grant, binds
every role to the creating agent at first, and cannot receive required
starting material until it exists. The page says so in one line and links to
the manual.

## 8. Work order

Each step is committed when its checks pass.

| Step | Work | Done when |
| --- | --- | --- |
| 1 | Test cases and vectors export: cases file, `check_blueprints.py` runs and checks them | `check_blueprints.py` passes; every code has a case |
| 2 | TypeScript checks (section 5) with tests against the vectors | `npm test` passes on every vector |
| 3 | Editor model: document, edits with reference updates, undo, presets, plain summary, page checks for hidden characters | Unit tests pass, including layout changes leaving the blueprint unchanged |
| 4 | Prompt builder, data blocks, extraction of pasted blocks, `docs/blueprint-prompt.md` | Prompt tests pass; the generated prompt's blocks validate with the real CLI |
| 5 | Canvas fork and stage map with settings panel | The map works in a production build |
| 6 | Page: ways of working with diagrams, rules panel, summary, problems, copy, saved blueprints, links, import | The six ways copy without edits; browser tests pass |
| 7 | Site integration: header link, `llms.txt`, site README, docs page under "Author definitions", sitemap | Site checks pass |
| 8 | Browser walkthrough of the first visit, a preset, a pipeline and copying, from a first-time user's point of view; fixes | Findings recorded in the research note |

Status: steps 1 to 7 are done (commits `17f9c96` to the site integration commit).
Step 8 has started: a walkthrough in the browser found and fixed a misleading
change count, an awkward arrow label, the way cards' grid and the panel
covering the map.

Testing with real first-time users is for the owner to run; the research note
holds the script.

## 9. Checks

- `npm run lint`, `npm run check`, `npm test`, `npm run build` and the
  Playwright tests in `sites/locust.farm/`.
- `python3 scripts/check_blueprints.py` for exports and vectors.
- `cargo fmt`, `cargo clippy` and `cargo test` when Rust changes.
- `python3 scripts/check_docs.py` for documents.
- An automated end-to-end test copies the prompt from a production build,
  extracts the blueprint block and runs `locust blueprint validate` on it.

## 10. Risks

| Risk | Answer |
| --- | --- |
| The TypeScript checks drift from Locust | Generated vectors; the export check fails on a code without a case; the agent re-checks with Locust |
| People read arrows as automatic execution | The fixed sentence in section 3.5; arrows say "when … is complete" |
| The six ways do not fit someone's team | Every choice stays editable; "Open a blueprint" accepts anything an agent writes |
| Pasted prompts get cut or altered | Byte count, SHA-256, END lines and a final line |
| Names or advice try to instruct the agent | They appear only inside data blocks; the prompt says they are data |
| Polaris and the site drift apart | Accepted; the fork records its source commit |
