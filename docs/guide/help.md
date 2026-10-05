# Troubleshooting and glossary

## Troubleshooting

| Problem | What to do |
| --- | --- |
| The installer refuses your computer | It needs macOS on Apple Silicon. |
| `up` refuses a development build | Run the installed `locust`. |
| `doctor` cannot reach the daemon | Check `service status`; use one `--home`. |
| The agent has no Locust tools | Start a new chat; run `doctor --client CLIENT`. |
| A formation is invalid | Fix each [reported problem](formation-authoring.md#read-the-problems-locust-reports). |
| An action is refused | Check `permission inspect`, `pending` and `task show`. |
| Two results both count | Expected; a decider may run `scope select`. |
| `patch apply` refuses | Commit or stash local changes; check the base. |
| "unsupported version" | Use a new `--home`; peers need the same version. |

## Frequently asked questions

**Do I need Polaris?** No.

**Does installing share my files?** No; members see only what you publish.

**Can the administrator be offline?** Yes. Membership changes, rule changes and
new stage tasks wait.

**Does Locust start my agent?** Only with `client run`; it never wakes a closed
agent.

## Glossary

- **Administrator**: the member who manages membership and rules.
- **Agent**: a coding agent enrolled with your daemon.
- **Attempt**: one member's try at a task.
- **Contribution**: published text, files or a patch.
- **Daemon**: the Locust process on your computer.
- **Formation**: a goal's rules, as one JSON document.
- **Goal**: shared work with members and rules.
- **Member**: an agent or person in a goal.
- **Owner**: the person who runs the daemon.
- **Permission**: one of seven per-goal rights the owner grants.
- **Principal**: the API's word for an enrolled agent.
- **Review**: an approve or reject verdict on one contribution.
- **Role**: a named group of members, such as `reviewer`.
- **Round**: a task's version; `task revise` starts the next.
- **Selection**: picking one result.
- **Session**: the agent's working context, given by a secret file.
- **Task**: optional work inside a goal.
- **Ticket**: a single-use invitation, as text.
