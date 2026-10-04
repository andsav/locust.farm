# Blueprint editor review: first-time understanding

Date: 2026-10-04. **Status: review findings and a proposal. Nothing here is
accepted or implemented.** It reviews the editor built from the
[blueprint editor plan](../docs/blueprint-authoring-plan.md) at commit
`9e68cf5`, and extends the [blueprint authoring research](blueprint-authoring.md).

Labels: **[V]** checked by the reviewer in source or in the running page;
**[R]** reported by a review agent with a source citation, not re-checked;
**[I]** inference or recommendation.

## Question

The owner asked whether the editor is simple and obvious for someone who knows
agentic coding but has had no Locust onboarding, whether the rule choices
(Start, Done and the stage options) are the right primitives, and what to
change. The aim is a page that makes a person's understanding of how their team
works explicit and visible, so they can find arrangements beyond the presets.

## Short answers

1. The six animated cards work. Everything below them is harder than it needs
   to be, mostly because meaning sits in hover text and the large drawing is
   not drawn from the rules.
2. The primitives are close, but cut in the wrong places. "Start" promises
   ownership the runtime does not give. "Done" names the wrong unit and allows
   one condition where people expect to combine them. The choice that defines
   two of the six presets, who picks one result, is hidden under More.
3. The six presets are four answers. Each differs from Open at one or two of
   four points in the life of a task: who adds it, who works on it, when a
   result counts, whether one result is picked. Showing those four points as
   one line, with a drawing and a short phrase at each, replaces the rail, the
   More panel and the decorative drawing, and lets a person mix answers.
4. Several things the page says are not what Locust does today. Those need
   either a runtime change or different words.

## Method and limits

- Reading the editor's source, the plan and the earlier research. [V]
- A walkthrough of the dev server in Chromium at 1440×900 and 390×844 with the
  site's Playwright: first visit, each card, stage settings, More, In words,
  the prompt, rule combinations. 20 screenshots. [V]
- Two cold reads by review agents that saw only the screenshots, then the
  hover text. One played a solo developer with four agent sessions, one an
  engineering lead who thinks in pull requests and CI. They attempted six
  tasks. **These are role-plays by a model, not tests with people.** They are
  useful for finding likely confusions, not for measuring them.
- One agent read the Rust sources to say what each format field does at run
  time. The reviewer re-checked the claims this note depends on; those are
  marked [V].
- One agent listed every visible string and control in the editor.
- Three agents each proposed a model from a different starting point, and one
  agent tried to break all three. The proposal below is the reviewer's
  synthesis.

The owner's script for real first-time users in the
[authoring research](blueprint-authoring.md) still applies and is still not run.

## Findings

### 1. What the runtime does differs from what the page says

| The page says | The runtime does | Source |
| --- | --- | --- |
| "When is a task done?" | The done rule is judged per posted result. A task can have several results that each count; the task shows completed after the first. | [projection.rs](../crates/locust-core/src/goal/projection.rs) lines 190 to 223 [V] |
| Start: "Anyone" / "Handed out" decides who does the work | Posting a result checks only the publish rule. An attempt is optional. Start rules decide who may record an attempt or send an offer, not who may deliver. | [fold.rs](../crates/locust-core/src/goal/fold.rs) lines 277 to 303 [V] |
| "Hand the task out instead" (under things Locust can't do, for first to grab keeps it) | An offer is not exclusive. Exclusive reservation is deferred. | [semantics](../docs/organization-blueprints-semantics.md) line 44 [V] |
| Start: "Nobody" is one of three main ways to start | With no start rules people can still create tasks and post results. Only attempt records and offers go away. No preset uses it. | [organization.rs](../crates/locust-proto/src/organization.rs) [R] |
| "Finish goal: who can say the whole goal is finished" | Closing the goal scope is recorded and changes nothing. Closing a task refuses new attempts. | [projection.rs](../crates/locust-core/src/goal/projection.rs) lines 382 to 404 [V] |
| Advice is "shown to people and agents in the goal" | `guidance` is read nowhere outside the format type. | `grep guidance crates` [V] |
| Check: "a named automated check, such as the tests" | A member reports a named check as passed on one result. Nothing is run. The caveat is only in hover text. | [fold.rs](../crates/locust-core/src/goal/fold.rs) [R] |
| Pipeline: draft, then review | The review stage creates a second, separate task with no title. Someone posts a new result there and a peer approves that new result. The draft is not reviewed and is not attached. | [flow.rs](../crates/locust-core/src/goal/flow.rs) lines 25 to 85 [V], [pipeline.json](../examples/blueprints/pipeline.json) |
| Roles are filled "when you start a goal" | At creation only the creator is a member, so every single-person role is the creator, and a two-reviewer rule cannot be met until rules are bound again. | [goals.rs](../crates/locust-core/src/node/requests/goals.rs) lines 70 to 116 [V] |
| Stages run | Stages create nothing until the runner's Locust has the local `flow` grant. A creator gets only `administer`. | [node/flow.rs](../crates/locust-core/src/node/flow.rs) lines 11 to 30 [V] |

Two more from the source reading, not re-checked: giving a stage its own done
rule copies the current picker and closer into a task type, so later changes to
them do not reach that stage ([edit.ts](../sites/locust.farm/src/lib/blueprint-editor/model/edit.ts)
lines 430 to 438 [V] for the copy); and no runtime test covers `all`, `any` or
`check` completion [R].

### 2. The choice that defines a preset is hidden

Open and Independent attempts show the same rail: Start Anyone, Done Says so.
The only visible difference is a "judge" chip. The rule itself, who picks the
final answer, is under More. Coordinator's picking and finishing are also only
under More. [V] Both cold readers failed to find where "a judge picks" is set.

### 3. The large drawing is not drawn from the rules

[pictureFor](../sites/locust.farm/src/lib/blueprint-editor/model/presets.ts)
picks one of six fixed animations by priority. [V] Seen in the walkthrough:

- Start "Handed out" by Everyone draws a hub labelled "coordinator" when no
  such role exists, under the card "Open changed".
- Start "Nobody" with Done "Review" plays the peer review animation.
- Review panel draws three reviewers and "2 of 3 approved" for any number,
  which reads as a majority vote. The page lists majority votes as something
  Locust cannot do.
- The highlighted card is the last one clicked, not the one the rules match.

Both cold readers stopped trusting the drawing after two mismatches. Both
asked for a legend, for the drawing to use their role names and numbers, and
for a way to follow one task through the rules.

### 4. Meaning is only in hover text

At rest the rail shows eleven words: Roles, Start, Anyone, Handed out, Nobody,
Done, Says so, Review, Check, More. The questions ("How does work start?",
"When is a task done?") and every consequence are in hover text. The inventory
found 33 meanings available only on hover and 34 controls with no visible
label. Hover text does not appear on touch screens
([tooltip.ts](../sites/locust.farm/src/lib/blueprint-editor/ui/tooltip.ts)). [R]

The fields under a chosen option are an icon and a value: three look-alike
dropdowns (who hands out, who reviews, who reports) and a bare number. [V]

Both cold readers named "In words" as the panel they wanted from the start. It
is behind an unlabelled icon.

### 5. Words

- One word, several meanings: "Check" (done rule, prompt mode, problems
  button, and a tick for saved); "+" and "Add" (role, stage, condition, prompt
  mode); "Start" (site navigation, how work starts, Starts after, start a
  goal, Start over, starting material); "Open"; "draft"; "review" (a done rule
  and a stage name); "accept" (take offered work, approve a result). [R]
- One thing, several words: task, work, stage, step, attempt; result,
  findings, answer; done, complete, finished, counts. A new stage is named
  "step". [V]
- "Run by" and "runner" read as the agent or CI runner that does the work.
  One cold reader read "Run by" and "Work goes to" the wrong way round.
- "Anyone" under Start was read by both cold readers as claiming a task.
  Preventing two agents doing the same task is what the solo developer came
  for, and the page says it is not possible only in a collapsed list at the
  bottom of More.
- The page says person and people throughout. Neither reader could tell
  whether one of their agent sessions is a participant. The guide says a
  participant is a Locust identity and an agent harness works for it
  ([concepts](../docs/guide/concepts.md)). If one person's four sessions are
  one participant, "not the author" does nothing for a solo developer. [I]

### 6. Stages

- The stage panel's Done section is below the fold at 1440×900, so the
  Pipeline card's promise that each step can have its own rules cannot be
  found. [V]
- The round "+" adds an unconnected stage named "step", numbered 2, with
  "review" renumbered 3. Problems reports nothing. [V]
- The first "+" silently adds a "runner" role. [R]
- The rail's Start and Done stay on screen with stages loaded; nothing says
  they are the default for every stage. [V]
- A prerequisite of kind "has a review" can never be met when the earlier
  stage's done rule has no reviews; nothing warns. [R]

### 7. Smaller defects

The approvals box accepts 0, which Locust rejects. "Work goes to: Nobody" and
"Share findings: Nobody" are offered and leave a blueprint that cannot make
progress. The disabled "Own" option takes the only tab stop, so the keyboard
cannot enter that row. The side panel draws an empty footer strip that cuts
content. The toolbar's "+ Add / Check" switch changes what Copy prompt copies
with no visible link to it; one cold reader would have copied a check-only
prompt by mistake. On a phone the drawing is a full screen below the controls.
[R, footer and phone layout V]

### 8. The cold readers' tasks

| Task | Solo developer | Team lead |
| --- | --- | --- |
| Three agents attempt one fix, I pick | guessing | guessing |
| Two approvals for every result | confident | confident |
| Tests pass and another agent approves | stuck | stuck |
| A planner assigns tasks | confident | confident |
| Plan, implement, review, ship, one role each | guessing, would get it wrong | guessing |
| See what it will do before copying | guessing | guessing |

"Tests pass and a review" is the most common rule both know. The format can
express it (`all`); the page offers one of three.

## Proposal: one line with four points

A blueprint answers four questions in the order a task lives. Each preset is
Open with one or two answers changed.

| Point | Question shown | Answers | Today |
| --- | --- | --- | --- |
| Add | Who adds tasks? | Anyone. A role. No tasks, members only post results. | More: Suggest tasks; Start: Nobody |
| Work | Who works on a task? | Anyone, with "No lock: two members can work on the same task" shown. A role. A role asks a member, who can say no. | Start: Anyone, Handed out |
| Counts | When does a result count? | A list that combines: N approvals from a group, not the author's; a named check reported as passed. Nothing ticked: when posted. | Done: Says so, Review, Check; Own |
| Pick | Is one result picked? | Nobody, all results that count stay. A role picks one result per task. | More: Final answer |

| Preset | Add | Work | Counts | Pick |
| --- | --- | --- | --- | --- |
| Open | anyone | anyone | author says so | nobody |
| Coordinator | anyone | coordinator asks | 1 approval, coordinator | coordinator |
| Peer review | anyone | anyone | 1 approval, not the author's | nobody |
| Review panel | anyone | anyone | 2 approvals, reviewers | nobody |
| Independent attempts | anyone | anyone | author says so | judge |
| Pipeline | the same line, twice, in order | | | |

The page:

```
How does your team work?
[Open][Coordinator][Peer review][Review panel][Independent attempts][Pipeline]

Name ______   Undo  Redo  File                               [COPY PROMPT]
                                Paste into your coding agent. Needs Locust.
 any task
  WHO ADDS TASKS   WHO WORKS ON ONE   WHEN A RESULT COUNTS   IS ONE PICKED
  (drawing) -----> (drawing) -------> (drawing) -----------> (drawing)   + step
  Anyone.          Anyone. No lock.   1 approval, not        Nobody. All
                                      the author's.          results stay.
  + another kind of task
 Roles: none. Filled later, in Locust.
 Locust records these rules. It does not start agents or run checks.
```

- The six cards stay as drawn. The lit card is the one whose rules equal the
  current rules, or none.
- Each point is a small drawing with one phrase under it. The drawings reuse
  the card motifs with real values: the mesh or the hub for Work, as many
  reviewer circles as approvals for Counts, the judge for Pick, the role's own
  name on a named circle. They play in turn, left to right, so the line shows
  one task moving through the rules. The four phrases read as one sentence, so
  the "In words" panel is no longer needed for the common case.
- A click on a point opens a small box under it with full-phrase options and a
  word on every field. Every list of members ends with "New role", so roles
  are made where they are needed. The limit that belongs to a setting is
  printed in its box ("Locust does not run the check").
- "+ step" adds another line below, already joined. Its first point reads
  "Locust adds this task once, after draft has a result that counts". Steps
  are rows in order; no free canvas, no zoom, no numbering.
- "+ another kind of task" adds a line for tasks that follow other rules. It
  replaces Task types and "Own rule".
- One line near Copy prompt says what happens next. The Add / Check switch
  becomes a text link.

Why this fits the owner's aim [I]: it keeps the drawings, and it makes the
space of arrangements visible. A person sees that the hub, the reviewer fan
and the judge are answers to different questions and can combine them, for
example a coordinator who asks, two approvals and a judge, which no card
shows.

Words to change, whatever the layout [I]: "When does a result count?" for
"Done"; "approvals" with words on each field; "check reported as passed" with
the caveat visible; "No lock" beside Anyone; "member" for person, with one
sentence saying what a member is; "step" or "stage", one of them; remove
"runner" and "Run by" from the page.

## Changes that need no redesign

In rough order of value [I]:

1. Show the two questions on the rail and put a word on every field.
2. Move "who picks one result" onto the rail.
3. Light the card whose rules match; draw role names and approval counts from
   the rules; remove "2 of 3 approved".
4. Put the stage's Done section first in its panel; make "+" add a connected
   stage; drop the numbers or number by order.
5. Replace the Add / Check switch; rename one of the "Check"s.
6. Correct the statements in finding 1 that no runtime change will make true.
7. Fix the defects in finding 7.

## Format and runtime changes worth considering

1. `runner` optional, defaulting to the goal's administrator, who also gets the
   `flow` grant. Removes the runner role and the case where stages silently
   never start. Reverses the accepted statement that the runner is "not a
   default universal coordinator" ([semantics](../docs/organization-blueprints-semantics.md)),
   so it needs an owner decision.
2. A stage's task carries the stage name as its title and a link to the result
   that triggered it.
3. Change the bundled pipeline to draft (one approval) then ship; a stage named
   review does not review the draft.
4. A signed-replay test for `all` and `check`, which the combining list needs.
5. Either show `guidance` to agents or remove it; either give closing a goal an
   effect or remove `finish` at goal level.
6. A publish rule for "members working on the task", so Work is a real gate.
7. Exclusive claim. Deferred in the accepted documents, and what the solo
   developer came for.

## Ideas considered and not recommended

- A page-side simulator that plays any rule combination truthfully: a second
  copy of the rules engine that would drift from Locust.
- A "people shown" counter on the drawing: looks like a setting, saves nothing.
- Framing tasks and results as issues and pull requests with branch
  protection: familiar, but it imports enforcement Locust does not have.
- "Takes" and "assigns" for the Work point: both promise ownership.
- Generating the six cards from rules: they are fixed presets and the
  hand-drawn ones are the part that works.

## Open questions for the owner

1. Is a member one person with all their agents, or can each agent session be
   a member? The answer decides what "not the author" means and what a circle
   in the drawings stands for.
2. May the administrator run stages by default?
3. Should a step happen once per goal, as today, or once per task? Developers
   mean the second by "pipeline", and only the second lets a review step judge
   the earlier result.
4. Is an exclusive claim in scope? Until it is, the page should say "No lock".
5. May "author says so" leave the main choices, so a result counts when posted?
6. "Asks", or Locust's own word "offers"? The person's agent will say "offer".
7. Keep the free stage canvas for flows that branch, or use rows in order?
8. How does an agent learn there is work for it? The page needs one sentence.
