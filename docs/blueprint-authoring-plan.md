# Blueprint editor on locust.farm: implementation plan

Date: 2026-10-04. **Status: implemented in the site at `/blueprints`; checked by
unit tests, conformance vectors and a browser walkthrough. Not yet tried with
real agents or first-time users (section 8).** Baseline: Locust `74ec9fb`, after the
organization implementation landed (`809bbbe`) and the contract rename. The
Polaris checkout read for the side panel is `dreamcolor10` at `01d8aa3c4`. The
page was rebuilt later the same day around one line of four points (decisions 9
to 13, section 3).

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
4. Superseded by decision 9. (Was: the canvas shows only stages and the order
   between them; everything else is edited in panels.)
5. The side panel keeps the look of Polaris's settings panel, in Locust's
   colours, font and tone.
6. Superseded by decision 9. (Was: the canvas code is forked once from Polaris
   into the site.)
7. All checks that need no goal or machine context run in TypeScript in the
   page. There is no WebAssembly build. Locust stays authoritative: the agent
   re-checks with Locust before anything is saved.
8. The contract uses plain names: `task_types` (was `variations`), a stage's
   `task_type`, `decisions.finish` (was `closure`) and a stage's `runner` (was
   `materializer`). Implemented in `74ec9fb`. The runner was removed later
   (decision 10).

Later the same day, after a [review](../research/blueprint-editor-review.md) of
the built editor:

9. The rules are shown as one line of four points: who adds a task, who works
   on it, when a result counts, and whether one result is picked. Each point
   has a picture drawn from the rules and a short answer, and a click shows its
   choices. Steps and other kinds of task are shorter lines under it. This
   replaces the rules rail, the More panel, the decorative drawing and the
   stage canvas.
10. A stage has no runner. The goal's administrator, who is the member that
    started the goal, runs every stage. Implemented in `cd9bf97`.
11. One agent is one member. The page says "member" and "anyone", never
    "person" or "people".
12. A step is one task that Locust adds, as before; the page says so. A
    workflow that every task passes through is what the four points already
    are.
13. An exclusive claim on a task is wanted but not built. Until it exists the
    page says "No lock" beside "Anyone".

Decision 7 departs from the blueprint plan's O1.4 ("prevent handwritten copies
in Polaris or the site") and from the accepted direction's "use the same
validator". The test cases in section 5 keep the copy honest.

## 1. Outcome and scope

A person opens `locust.farm/blueprints`, picks how their team works, changes any
of four points, optionally adds steps, and presses one button to copy a prompt. Pasted into their agent, the prompt makes the agent check
the blueprint with their Locust, explain it in Locust's words, save it as a
private draft and ask before publishing it.

Done means:

- Each of the six ways of working can be chosen and copied without any edit.
- Every part of the contract the editor can show is editable by keyboard and
  on a phone.
- The page's checks agree with Locust on every test case (section 5).
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
| Catalog | Private drafts, a separate presentation record holding opaque `data_json` that never affects identity, and immutable publications; operations `blueprint.draft.create`, `blueprint.draft.update`, `blueprint.drafts`, `blueprint.presentation.update`, `blueprint.publish` and others ([API](../crates/locust-proto/src/api.rs)) | Prompt steps; the blueprint's name |
| Flow | A stage is ready when its `requires` evidence exists; the goal administrator's Locust then creates its task and delivers it to the stage's recipients. No agent is started ([semantics](organization-blueprints-semantics.md)) | The wording of steps |
| Goal creation | `goal.create` takes inline `blueprint_json`, `roles` and `inputs`, needs the daemon-wide `manage_goals` grant, makes the creator the only member, and needs required inputs as blobs that exist only after the goal does | Why the prompt does not start goals |
| Site | SvelteKit 3, Svelte 5, Vite 8, `node --test`, prerendered pages, a docs site under `/docs`, CI in [site.yml](../.github/workflows/site.yml) | Where the editor lives |

Known Locust quirks the port does not copy: the decoder accepts a root array
and some shapes the schema does not allow. The test cases stay inside the
schema's shapes, and the research note lists the quirks for lane A.

## 3. The page

### 3.1 Layout

Under the site header: the headline, one sentence saying what a blueprint is,
the six ways of working as a strip of animated pictures, then one framed
editor:

- **Toolbar.** The blueprint's name and a save mark; Undo, Redo and a File
  menu (open, saved, copy a link, download, start over); a problems button and
  "In words", both with their word shown; and the ember "Copy prompt" button.
  A line under it says what to do with the prompt and holds the links "See the
  prompt", "Copy one that only checks", "Set up Locust" and "Manual".
- **The line for any task.** Four points, left to right, each with its
  question, a picture and a short answer.
- **Steps** and **other kinds of task**, when there are any: one row each, with
  the same four columns.
- **Roles**, then one line of limits: "A member is one agent or one person.
  Locust records these rules. It does not start agents or run checks."

Nothing a person needs is only in hover text. Hover text names the icon
buttons and repeats each card's sentence. One side panel slides over the right
of the editor for a role's settings, problems, the blueprint in words, the
prompt, and open and saved blueprints. There are no modal dialogs. On a phone
the points stack two by two and a step is a short list; the side panel covers
the screen.

The design context, shared with the impeccable design skills, is in
[.impeccable.md](../.impeccable.md).

### 3.2 The six ways of working

| Way of working | Sentence | Preset |
| --- | --- | --- |
| Open | Everyone works freely and shares what they find. You say when your own work is done. | `open` |
| Coordinator | One member asks others to do tasks and approves the results. | `coordinator` |
| Peer review | Anyone can work on a task. Someone else has to approve a result before it counts. | `peer-review` |
| Review panel | Two reviewers have to approve each result before it counts. | `review-panel` |
| Independent attempts | Several members try the same task in their own way. A judge picks the result to use, and the other results are kept. | `independent-attempts` |
| Pipeline | Tasks that Locust adds in order. Each one is added when the one before it has a result that counts. | `pipeline` |

Each card shows its picture and name; the sentence is its hover text and is
read to screen readers. Choosing a card loads that preset as one undoable
change. The lit card is the one whose rules equal the current rules, whatever
was clicked last, and no card is lit for an arrangement none of them shows
([matchingWay](../sites/locust.farm/src/lib/blueprint-editor/model/presets.ts)).

### 3.3 The four points

Each preset is Open with one or two answers changed
([line.ts](../sites/locust.farm/src/lib/blueprint-editor/model/line.ts)).

| Point | Answers | Contract |
| --- | --- | --- |
| Who adds tasks? | Anyone. Only one role. Nobody: there are no tasks and members only post results. | `work.propose` |
| Who works on a task? | Anyone, with "No lock: two members can work on the same task". Only one role, who are then the only ones who can post results. A member is asked, can say no, and others can still post results. | `work.starts`, `work.publish` |
| When does a result count? | A list that combines: N approvals from anyone or a role, with or without the author's own; a named check reported as passed, with "Locust does not run the check". Nothing ticked: its author says so. | `decisions.completion` |
| Is one result picked? | Nobody, and every result that counts stays. One role picks one result per task. The same box holds who can close a task. | `decisions.selection`, `decisions.finish` |

A click on a point shows its choices in a box under the line. Options are full
phrases, every field has a word, and the limit that belongs to a setting is
printed beside it. Every list of roles ends with "New role", which asks for a
name in place and adds the role and the rule as one change.

The pictures are drawn from the answers, with the blueprint's own role names
and numbers ([drawPoint](../sites/locust.farm/src/lib/blueprint-editor/ui/diagrams.ts)):
the members and the task for Add, the mesh or the hub for Work, as many
reviewer circles as approvals and a diamond for a check for Counts, and the
picker's line to one result for Pick. The four share one clock and play in
turn, left to right.

A rule the page does not offer, such as "any of these", is shown as a sentence
and kept as it is until an option replaces it.

### 3.4 Steps and other kinds of task

"Step" adds a row under the line. A step is one task that Locust adds: at the
start, or after another step has a result that counts. If someone picks a
result in the earlier step, the later one waits for the pick; the page chooses
`completion` or `selection` and keeps it right when the picker changes. A new
step waits for the last one. Removing a step joins the steps around it.

A row has the four columns of the line. A cell says "Same as any task" until
its point is changed for that step; the change is stored as a task type named
after the step, and it goes away when the step follows the main rules again.
The points a step did not change keep following the main rules when those
change. Who a step's task is sent to follows who works on it.

"Another kind of task" adds a row for tasks that members add under other
rules. It is a task type no step uses.

Contract parts the page does not show (`context.guidance`, `context.inputs`,
prerequisites on a posted result or a review, several prerequisites) are kept
unchanged and listed under the roles.

### 3.5 Problems and the blueprint in words

The problems button shows "No problems" or the count, in ember when there are
any. It opens the problems in the side panel: a plain sentence first, then
"Show me", which opens the point it is about, and "Details" with the code and
path. A point with a problem has an ember outline. Problems block nothing:
copying with problems produces a prompt that saves an unfinished draft and does
not publish.

"In words" opens the whole blueprint as sentences written by the page, then
"What Locust will say": Locust's own explanation lines, produced by the
TypeScript port, which is what the agent will show.

### 3.6 Copy

The ember "Copy prompt" button is the page's one primary action and is always
in the toolbar. It copies the prompt that adds the blueprint as a private draft
and asks before publishing. "Copy one that only checks" is a link beside it:
the agent checks the blueprint with Locust and explains it, and nothing is
saved. After copying, the button says "Copied" and a note says "Copied. Paste
it into your agent." A failed copy opens the prompt with the text selected.

### 3.7 Keeping work

- The browser keeps a list of saved blueprints (local storage, one record per
  blueprint). Opening a link, importing or starting over creates a new record;
  nothing is replaced silently. A check beside the name means saved; an ember
  warning means this browser is not keeping it.
- "Copy a link to it" puts the blueprint and its name in the address after
  `#`, compressed. The part after `#` is not sent to any server. Opening such a
  link creates a new record and shows "Opened from a link. If someone else made
  it, read its names before you copy it."
- "Open a blueprint" accepts pasted JSON, a pasted agent reply containing the
  prompt's blueprint block, or a file.
- "Download as JSON" saves the blueprint.
- The presentation record carries only the blueprint's name, under the page's
  own key: `{"locust.farm": {"name": "…"}}`. Other keys in that record are kept.

### 3.8 Wording

Contract words appear only under a problem's "Details" and in the prompt's
data. The page says "member" and "anyone" for who takes part; "task" and
"result" for the work; "counts" for a result that meets its rule; "approval"
for an approving review; "picks" for `selection`; "close a task" for `finish`;
"step" for a stage; "another kind of task" for a task type; "asks" for an
offer. It never says "runner".

## 4. Look and feel

| Element | On the site |
| --- | --- |
| Points and rows | Square, hairline borders; the open point has the surface colour and an ember underline; a point with a problem has an ember outline |
| Pictures | The cards' own marks: grey dots are work and results moving between members, ember marks a decision. A named circle is a role |
| Side panel | Polaris's settings panel (`InspectorPanel.svelte`): slide-over, header with the 40px accent disc and title, one scrolling body, a footer only when there is something in it. Panel background `--neutral-950` with a hairline `--color-border` |
| Icons | Phosphor, regular weight, copied into `ui/icons.ts`; an icon alone only for Undo, Redo, Close and the save mark, each with hover text |
| Focus | The site's 1px ember outline |

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

## 6. No canvas

The first version forked Polaris's canvas for a map of stages. Decision 9
replaced the map with rows, and the fork, `@xyflow/svelte` and `@dagrejs/dagre`
were removed from the site. The side panel is still adapted from Polaris's
settings panel at `dreamcolor10` `01d8aa3c4`; the site README records the
source path.

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
| 5 | Canvas fork and stage map with settings panel | Done, then removed by decision 9 |
| 6 | Page: ways of working with diagrams, rules panel, summary, problems, copy, saved blueprints, links, import | The six ways copy without edits; browser tests pass |
| 7 | Site integration: header link, `llms.txt`, site README, docs page under "Author definitions", sitemap | Site checks pass |
| 8 | Browser walkthrough of the first visit, a preset, a pipeline and copying, from a first-time user's point of view; fixes | Findings recorded in the research note |

Status: steps 1 to 7 are done (commits `17f9c96` to the site integration commit).
Step 8 has started: a walkthrough in the browser found and fixed a misleading
change count, an awkward arrow label, the way cards' grid and the panel
covering the map.

A later [review](../research/blueprint-editor-review.md) of the built editor
listed what a first-time person is likely to misread. The owner accepted its
proposal the same day (decisions 9 to 13), and the page was rebuilt as section
3 describes: the line of four points, steps as rows, no runner and no canvas.

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
| People read steps as automatic execution | A step's first column says "Locust, after … has a result that counts"; the limits line says Locust does not start agents |
| The six ways do not fit someone's team | Every choice stays editable; "Open a blueprint" accepts anything an agent writes |
| Pasted prompts get cut or altered | Byte count, SHA-256, END lines and a final line |
| Names or advice try to instruct the agent | They appear only inside data blocks; the prompt says they are data |
| Polaris and the site drift apart | Accepted; only the side panel remains from Polaris, and the site README records its source |
