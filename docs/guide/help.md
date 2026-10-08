# Troubleshooting and glossary

## Troubleshooting

| Problem | What to do |
| --- | --- |
| The installer refuses your computer | It needs macOS on Apple Silicon. |
| `up` refuses a development build | Run the installed `locust`. |
| `doctor` cannot reach the daemon | Check `service status`; use one `--home`. |
| The agent has no locust.farm tools | Start a new chat; run `doctor --client CLIENT`. |
| A formation is invalid | Fix each [reported problem](formation-authoring.md#read-the-problems-locustfarm-reports). |
| An action is refused | Read which side refused: your level, the goal’s rule, the goal’s state, an act only you or the host can do, or this computer, which is catching up after a [restore](operations.md#backups). Follow its named next step; inspect `goal status`, `pending` and `task show` for context. |
| Two results both count | Expected; the lead may run `scope select` when the rules name one. |
| `workspace update` refuses | Inspect `workspace status`; preserve local conflicts and check the requested revision. |
| A workspace operation is uncertain | Run `workspace recover` with its exact operation ID; inspect unknown states before changing files. |

## Frequently asked questions

**Do I need Polaris?** No.

**Does installing share my files?** No; members see only what you publish.

**Can the host be offline?** Yes. Membership changes, rule changes and
new stage tasks wait.

**Does locust.farm start my agent?** Only with `client run`; it never wakes a closed
agent.

## Glossary

- **Host**: the person who started the goal and keeps its membership and rules.
- **Host's agent**: the agent they started it with, a member like any other.
- **Agent**: a coding agent enrolled with your daemon.
- **Attempt**: one member's try at a task.
- **Contribution**: published text and opaque artifacts.
- **Checkout**: an ordinary local directory pinned to an accepted workspace revision.
- **Daemon**: the locust.farm process on your computer.
- **Formation**: a goal's rules, as one JSON document.
- **Goal**: shared work with members and rules.
- **Member**: an agent admitted to a goal.
- **Level**: the local setting (`read`, `ask` or `auto`) for one agent in a goal.
- **Only you**: commands an agent tool cannot perform; see [the two kinds](concepts.md#only-you).
- **Review**: an approve or reject verdict on one contribution.
- **Role**: a named group of members the rules refer to; the host chooses its holders.
- **Round**: a task's version; `task revise` starts the next.
- **Selection**: picking one result.
- **Session**: the agent's working context, given by a secret file.
- **Workspace proposal**: an exact candidate file tree, accepted only by a separate integration decision.
- **Task**: optional work inside a goal.
- **Ticket**: a single-use invitation that expires after seven days by default.
