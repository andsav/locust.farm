# Local Codex and Claude demo

Status: implemented and [qualified locally](../research/demo-qualification.md)
with fresh native Codex/Claude clients and scripted loopback providers. This guide
starts after reviewed [installation](installation.md). It does not publish software or change an
existing goal's organization. Use fresh current-format state and a disposable
Git repository for the demo.

## Prepare the two clients

Use the installed `current/locust` executable below. Select the same explicit
`--home` on every command if the daemon does not use the default home. Keep the
chosen profile and workspace selections consistent when retrying onboarding.
In an interactive terminal these commands review the changes before applying:

```sh
locust up --client codex --name demo-codex --workspace /DEMO
locust agent add claude --name demo-claude --workspace /DEMO
locust doctor --client codex
locust doctor --client claude
```

For separate profile homes, supply `--profile-home` to onboarding and doctor,
and launch each client in that selected profile. Use `doctor --service none`
when observing a foreground daemon. Doctor infers the software prefix, enrolled
identity and workspace from the protected onboarding journal. Each failed
prerequisite gives a next step; it neither repairs files nor launches a client.
A successful check still reports native discovery unverified.

Start fresh Codex and Claude chats and ask each to read its installed Locust
skill and report its own Locust status. Confirm different principals and fixed
sessions. Their client approval policies and account configuration remain their
own. A second chat in one profile shares that profile's Locust identity/session;
it is not a third participant.

## Create the goal and authorize deliberately

Choose a blueprint explicitly. This demo uses peer review: another participant
must review a contribution before it qualifies. Creating a goal records its
administrator and gives that principal the existing goal-administration grant;
it does not give either agent daemon-wide goal-management authority.

```sh
locust --owner --as demo-codex goal create --title 'Demo change' --blueprint peer-review
locust --owner goal add-local --goal 'Demo change' --agent demo-claude --plan
locust --owner goal add-local --goal 'Demo change' --agent demo-claude --yes
locust --owner permission allow --goal 'Demo change' --agent demo-codex contribute
locust --owner permission allow --goal 'Demo change' --agent demo-claude contribute review
locust --owner --as demo-codex task open --goal 'Demo change' 'Implement the change

Make the requested change using the shared finding, publish its patch, and await peer review.'
locust --owner permission allow --goal 'Demo change' --agent demo-codex --task 'Implement the change' execute
```

`add-local` reviews whole-goal history/content sharing with an existing local
principal. It handles the signed local invitation internally without printing a
ticket. Repeating it observes existing membership. It adds no work grants. An
explicit owner admission may act for the local administrator without granting
that agent continuing authority; network admissions retain their ordinary
permission checks. If the administrator lives elsewhere, obtain a reviewed
[external invitation](guide/collaboration.md) instead.

Goal and task titles resolve only when unique in the caller's visible scope.
Use disambiguating identifiers when titles collide. Human event/contribution
selectors accept unique prefixes of at least eight hex characters. Machine API
and MCP requests retain full typed identifiers.

## Demonstrate useful collaboration

Give Claude a concrete investigation whose result affects the implementation,
then ask it to publish the finding through Locust. Give Codex the task objective
and ask it to inspect current shared context before implementing. The demo should
show the finding being read and acknowledged, its influence on the patch, and its
signed source attribution. Do not relay the finding manually into Codex's chat.

Use explicit [snapshot and materialization](https://github.com/andsav/locust.farm/blob/673aad942365c7af827e77c298cfa8bec51046c9/docs/t2-workflow.md) operations so the
worker changes its own workspace and the original checkout retains unrelated
work. Ask Codex to publish the task-backed patch before reporting its attempt
completed. Ask Claude to inspect that exact contribution's diff and independently
check it, then record a justified review. A contribution's summary is not proof
that its checks ran.

Observe without consuming either agent's context:

```sh
locust --owner status
locust --owner board --goal 'Demo change'
locust --owner inbox
locust --owner --as demo-codex pending --goal 'Demo change'
locust --owner --as demo-claude watch --goal 'Demo change' --timeout-ms 0
locust --owner permission inspect --goal 'Demo change' --agent demo-codex
locust --owner contributions --goal 'Demo change'
```

## Inspect and apply the exact result

Use the contribution prefix shown in the contribution list. The signed
contribution supplies its exact patch and base; neither needs to be copied into
another flag. Reviewing does not select or apply anything:

```sh
locust --owner --as demo-codex patch review --goal 'Demo change' --subject CONTRIBUTION_PREFIX
```

The peer-review preset does not require one globally selected output. After
reviewing the result, the person explicitly chooses it for the original checkout:

```sh
locust --owner --as demo-codex patch apply --goal 'Demo change' --subject CONTRIBUTION_PREFIX --root /DEMO --expected-git-head FULL_CURRENT_COMMIT --local-choice
```

The root must have the expected Locust workspace binding. Application validates
the signed base, affected files and Git HEAD, preserves unrelated edits, and
records local integration. `--local-choice` is owner-only and does not create a
replicated approval or selection. Organizations requiring shared selection use
`patch select --subject CONTRIBUTION_PREFIX` under their declared authority,
then ordinary application. Run the project's actual checks in the resulting
checkout and show the diff. Acceptance, application and verification are separate
observations.

Finish by inspecting and revoking the demo's permissions as appropriate:

```sh
locust --owner permission revoke --goal 'Demo change' --agent demo-codex --task 'Implement the change'
locust --owner permission revoke --goal 'Demo change' --agent demo-codex contribute
locust --owner permission revoke --goal 'Demo change' --agent demo-claude contribute review
```

Revoking permission does not kill a native client. Stop only the demo's own
process/service when cleaning up. Preserve journals and originals if cleanup or
application recovery is incomplete.

## Evidence boundary

The CLI/core tests exercise the named local handoff, no-write review, repeated
membership, unchanged grants, ambiguous-name refusal and signed patch application
with unrelated work preserved. The installed-client campaign now starts from
`up`, checks selected-profile doctor, and exercises native discovery and workspace
commands through the generated launcher. Its scripted-provider execution is
separate from natural-language real-model selection and interactive human approval.
The combined route passed on candidate `527ada5`; see the
[exact results and limitations](../research/demo-qualification.md). Earlier results
retain their own candidate identity in the [release evidence](release-evidence.md).
