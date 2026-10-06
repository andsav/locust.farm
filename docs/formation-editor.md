# Formation editor

Status: built at `/formations` on locust.farm. Not yet tried with first-time users or real agents.

The editor is a web page where a person builds a [formation](formations.md) and
copies one prompt for their coding agent. The agent checks the formation with the
person's own locust.farm and can save it there. The code is in
[sites/locust.farm/src/lib/formation-editor/](../sites/locust.farm/src/lib/formation-editor/).

## What the page does

- It asks "How does your team work?" and offers six ways of working, one per
  preset: Open, Peer review, Steps in order (`pipeline`), Independent attempts,
  Review panel and Directed. A new document starts as Peer review. The lit card
  is the one whose rules equal the current rules. Approval by other members
  includes the only-member alternative; approvals from a role do not.
- Rules are shown as rows. Each row answers four questions: who adds tasks, who
  works on a task, when a result counts, and whether one result is picked.
- The first row is for any task. Each step (flow stage) and other kind of task
  (task type) gets its own row. A step's change to an answer is saved as a task
  type named after the step.
- Roles are added in place. Problems show as a count, each with a plain
  sentence, a "Show me" link and the code and path. Problems block nothing: a
  formation with problems is saved as an unfinished draft and not published.
- "In words" shows the formation as sentences, then the lines locust.farm will print.
- Parts the page cannot edit, such as `context.inputs`, `context.guidance` or a
  rule it does not offer, are kept unchanged and listed.
- Work is kept in browser storage. A share link carries the formation,
  compressed, after `#`. That part is not sent to the server but stays in browser
  history.
- "Copy prompt" copies one prompt: check the formation, explain it, save it as a
  private draft with its layout, and ask before publishing. A second prompt only
  checks. The fixed text is in [formation-prompt.md](formation-prompt.md).

## How it is built

| Folder | Holds |
| --- | --- |
| `model/` | The document, edits that keep references right on rename or removal, undo, presets and plain wording. `line.ts` reads and writes a row's four answers |
| `contract/` | A TypeScript port of locust.farm's offline checks: strict JSON, schema version, structure, rules, normalization and explanation |
| `prompt/` | Builds the prompt, and reads back a pasted prompt, an agent's reply or raw JSON |
| `storage/` | Saved formations and share links |
| `ui/` | Svelte components: the six cards, the rows, the choices under an answer, and one side panel |

A test runs the `contract/` port on every conformance vector that
[check_formations.py](../scripts/check_formations.py) generates with the CLI. It
must give the same validity, codes, paths, messages, corrections, normalized form
and explanation. For JSON and structure errors the page writes its own message.
The port does not compute semantic or source hashes.

The layout record holds the formation's name and which answers each step sets
for itself, under the key `locust.farm`. The side panel is adapted from Polaris's
settings panel; the [site README](../sites/locust.farm/README.md) records the
source.

## Design rules

- No canvas. Rules are rows, and a step is one more row.
- The page never talks to a daemon and has no accounts or server storage. The
  agent checks the formation with locust.farm before anything is saved.
- The prompt never starts a goal. Starting one is the person's own command;
  the person names the agent that becomes the host's agent and first member.
- Names and advice from the formation appear only inside the prompt's data
  blocks, which the agent is told to treat as data.
- Plain wording: "member", "task", "result", "counts", "picks", "step". Format
  words appear only in problem details and the prompt's data.

## Tests

- `npm test` in `sites/locust.farm/` runs the unit tests and the conformance
  test. CI runs it. The prompt test checks that each fixed sentence appears as a
  whole line in [formation-prompt.md](formation-prompt.md).
- `npm run test:e2e` runs the Playwright tests in `e2e/formations.spec.ts` on a
  production build. If `target/debug/locust` exists, they also validate the
  copied formation with it. CI does not run them.
- `python3 scripts/check_formations.py` checks the vectors and exports against
  the CLI.

## Open issues

From an earlier review of the editor:

- Reviews so far used a model playing a first-time reader, not real people.
- What still confuses readers is what locust.farm does: no lock on a task, a check
  that a member only reports, a reject that does not block, and steps that happen
  once per goal. The page says each beside the setting.
- A step's task has no title and no link to the result before it.
- Card sentences are only in hover text, and there is no way to set a number of
  attempts.
- The page says "asks" where locust.farm says "offer".
