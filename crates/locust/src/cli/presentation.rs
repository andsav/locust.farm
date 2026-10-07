//! Human views of authoritative API responses. Labels never act as identity.

pub(super) use locust_proto::api::safe;
use locust_proto::api::{
    Abilities, AgentView, DaemonStatus, GoalStatus, GoalSummary, Halt, Level, MemberView,
    Membership, PendingWork, Response, Rule, SessionState, SessionView, TaskView, Voice,
    WaitOutcome, WaitingForYou, WaitingKind, WorkItem, allow_command, level_command, quoted, short,
};
use locust_proto::event::{Body, Scope, TaskId};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::organization::{CompletionRule, Formation, Selector};

/// Who reads a view, and what the daemon already told them so that names
/// print as the reader knows them and identifiers print short and complete.
#[derive(Clone, Copy)]
pub(super) struct Reader<'a> {
    pub voice: Voice,
    /// The agent whose view it is: the caller, or the one named with --agent.
    pub principal: Option<PublicKey>,
    /// Every enrolled agent, for local names.
    pub names: &'a [AgentView],
    /// Every goal this daemon holds, to cut a goal's identifier.
    pub goals: &'a [GoalId],
    /// The goal's members, for their names in it.
    pub members: &'a [MemberView],
    /// The goal's tasks, to cut a task's identifier and name its title.
    pub tasks: &'a [TaskView],
    pub now_ms: u64,
}

impl<'a> Reader<'a> {
    pub(super) fn new(voice: Voice, principal: Option<PublicKey>, names: &'a [AgentView]) -> Self {
        Self {
            voice,
            principal,
            names,
            goals: &[],
            members: &[],
            tasks: &[],
            now_ms: 0,
        }
    }

    /// The local name of the agent whose view this is.
    fn local_name(&self) -> Option<&'a str> {
        let principal = self.principal?;
        self.names
            .iter()
            .find(|agent| agent.agent == principal)
            .map(|agent| agent.name.as_str())
    }

    /// "you" for the person; "NAME's owner" for an agent.
    fn owner(&self) -> String {
        owner_words(self.voice, self.local_name())
    }

    fn goal_id(&self, goal: GoalId) -> String {
        short(
            &goal.to_string(),
            &self
                .goals
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
        )
    }

    fn task_id(&self, task: TaskId) -> String {
        short(
            &task.to_string(),
            &self
                .tasks
                .iter()
                .map(|task| task.task.to_string())
                .collect::<Vec<_>>(),
        )
    }
}

fn owner_words(voice: Voice, local_name: Option<&str>) -> String {
    match (voice, local_name) {
        (Voice::Person, _) => "you".into(),
        (Voice::Agent, Some(name)) => format!("{}'s owner", safe(name)),
        (Voice::Agent, None) => "your owner".into(),
    }
}

pub(super) fn utc(ms: u64) -> String {
    let seconds = ms / 1_000;
    let days = (seconds / 86_400) as i64;
    let hour = (seconds % 86_400) / 3_600;
    let minute = (seconds % 3_600) / 60;
    // Gregorian civil date from days since the Unix epoch.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

pub(super) fn expires_in(expires_ms: u64, now_ms: u64) -> String {
    let left = expires_ms.saturating_sub(now_ms);
    if left == 0 {
        return "expired".into();
    }
    const DAY: u64 = 86_400_000;
    const HOUR: u64 = 3_600_000;
    if left >= DAY {
        let days = left / DAY;
        format!("in {days} {}", if days == 1 { "day" } else { "days" })
    } else {
        let hours = left.div_ceil(HOUR);
        format!("in {hours} {}", if hours == 1 { "hour" } else { "hours" })
    }
}

pub(super) fn counts_when(rule: &CompletionRule) -> String {
    format!("A result counts when {}.", counts_clause(rule))
}

fn counts_clause(rule: &CompletionRule) -> String {
    match rule {
        CompletionRule::Declaration { by } => match by {
            Selector::ContributionAuthor => "its author says so".into(),
            _ => format!("{} says so", selector_words(by)),
        },
        CompletionRule::Contribution { by } => format!("{} posts it", selector_words(by)),
        CompletionRule::Reviews {
            by,
            count,
            exclude_author,
        } => {
            let from = match by {
                Selector::Members => String::new(),
                Selector::Role { name } => format!(" from \"{}\"", safe(name)),
                _ => format!(" from {}", selector_words(by)),
            };
            format!(
                "it has {count} approval{}{}{}",
                if *count == 1 { "" } else { "s" },
                from,
                if *exclude_author {
                    ", not the author's"
                } else {
                    ""
                }
            )
        }
        CompletionRule::Check { name, .. } => {
            format!("the check \"{}\" is reported as passed", safe(name))
        }
        // An "or" inside an "and" is bracketed, as the website prints it.
        CompletionRule::All { rules } => rules
            .iter()
            .map(|rule| match rule {
                CompletionRule::Any { .. } => format!("({})", counts_clause(rule)),
                _ => counts_clause(rule),
            })
            .collect::<Vec<_>>()
            .join(" and "),
        CompletionRule::Any { rules }
            if rules.len() == 2
                && rules.iter().any(|rule| {
                    matches!(
                        rule,
                        CompletionRule::Contribution {
                            by: Selector::OnlyMember
                        }
                    )
                }) =>
        {
            format!(
                "{}, or the goal's only member posts it",
                rules
                    .iter()
                    .filter(|rule| !matches!(
                        rule,
                        CompletionRule::Contribution {
                            by: Selector::OnlyMember
                        }
                    ))
                    .map(counts_clause)
                    .collect::<Vec<_>>()
                    .join(" or ")
            )
        }
        CompletionRule::Any { rules } => format!(
            "one of these is true: {}",
            rules
                .iter()
                .map(counts_clause)
                .collect::<Vec<_>>()
                .join("; ")
        ),
    }
}

fn selector_words(selector: &Selector) -> String {
    match selector {
        Selector::OnlyMember => "the goal's only member".into(),
        Selector::Members => "any member".into(),
        Selector::Role { name } => format!("members in the \"{}\" role", safe(name)),
        Selector::Participant { key } => format!(
            "one specific member (key {}…)",
            safe(&key.chars().take(8).collect::<String>())
        ),
        Selector::TaskCreator => "the member who added the task".into(),
        Selector::ContributionAuthor => "the author of the result".into(),
        Selector::Any { selectors } => selectors
            .iter()
            .map(selector_words)
            .collect::<Vec<_>>()
            .join(" or "),
        Selector::Nobody => "nobody".into(),
    }
}

fn label(key: PublicKey, names: &[AgentView]) -> String {
    names
        .iter()
        .find(|agent| agent.agent == key)
        .map(|agent| format!("{} ({key})", safe(&agent.name)))
        .unwrap_or_else(|| key.to_string())
}

/// A name another member chose, as it prints: bare when it is plain, else in
/// double quotes with `"` and `\` escaped, so that a name cannot imitate the
/// separators of a line and seem to hold a role.
pub(super) fn chosen_name(name: &str) -> String {
    let plain = !name.is_empty()
        && !name.starts_with(' ')
        && !name.ends_with(' ')
        && !name.contains("  ")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'));
    if plain {
        name.to_owned()
    } else {
        format!(
            "\"{}\"",
            safe(&name.replace('\\', "\\\\").replace('"', "\\\""))
        )
    }
}

pub(super) fn member_label(key: PublicKey, members: &[MemberView]) -> String {
    member_label_noting(key, members, "")
}

/// A member's label with a note inside its parentheses, such as
/// `Maple (02020202, the host's agent)`. The key prefix grows past eight
/// characters when another member's key shares them.
pub(super) fn member_label_noting(key: PublicKey, members: &[MemberView], note: &str) -> String {
    let prefix = super::selectors::member_key_prefix(members, key);
    let note = if note.is_empty() {
        String::new()
    } else {
        format!(", {note}")
    };
    members
        .iter()
        .find(|member| member.member == key)
        .map(|member| format!("{} ({prefix}{note})", chosen_name(&member.name)))
        .unwrap_or_else(|| format!("{prefix}{note}"))
}

/// A record's signer: the host's computer prints as `host` with no key.
fn signer(by_host: bool, key: PublicKey, names: &[AgentView]) -> String {
    if by_host {
        "host".into()
    } else {
        label(key, names)
    }
}

/// The `Host:` line of a goal: this computer, the host's agent once its
/// record is held, or until then the name the ticket gave, marked as the
/// ticket's word.
fn host_line(view: &GoalStatus, reader: &Reader) -> String {
    let host = view.host.map(|key| member_label(key, &view.members));
    if view.hosted_here {
        format!(
            "Host: {}{}",
            reader.owner(),
            host.map(|name| format!(" · {name}")).unwrap_or_default()
        )
    } else {
        format!(
            "Host: on another computer · {}",
            host.unwrap_or_else(|| match &view.host_name {
                Some(name) => format!(
                    "the ticket names {}; not confirmed until admission arrives",
                    chosen_name(name)
                ),
                None => "name has not arrived".into(),
            })
        )
    }
}

fn tag(value: &impl serde::Serialize) -> String {
    serde_json::to_value(value)
        .expect("API enum serializes")
        .as_str()
        .expect("API enum uses a string tag")
        .replace('_', " ")
}

fn scope(scope: Scope) -> String {
    match scope {
        Scope::Goal => "goal".into(),
        Scope::Workspace => "workspace".into(),
        Scope::Task(task) => task.to_string(),
        Scope::Document(doc) => format!("{} document", tag(&doc)),
    }
}

fn halt(reason: Halt) -> &'static str {
    match reason {
        Halt::AuthorityConflict => {
            "Blocked: conflicting authority history. Inspect the retained events with the goal host; decisions cannot advance."
        }
        Halt::SignerRecovery => {
            "Catching up: nothing is signed here until this computer has caught up."
        }
        Halt::SignerConflict => {
            "Conflict: two of this agent's records sit at one position. It signs nothing more here."
        }
    }
}

/// What an agent that is not a member of a goal can do about it; `owner` is
/// "you" or "NAME's owner".
fn membership_action(membership: Membership, owner: &str) -> Option<String> {
    match membership {
        Membership::Joining => Some(format!(
            "Admission has not arrived. It comes from the host's computer when that computer is on; nothing here waits for {owner}."
        )),
        Membership::Refused => Some(
            "The invitation was refused. Ask the goal host for a fresh invitation, inspect it, and join again.".into(),
        ),
        Membership::Removed => Some(
            "This participant was removed. Ask the goal host about readmission; changing local permissions does not restore membership.".into(),
        ),
        Membership::Left => Some(
            "This participant left. To participate again, obtain and inspect a fresh invitation, then join again.".into(),
        ),
        Membership::Member => None,
    }
}

/// What an agent does in a goal at its level, and who is waited for at ask.
pub(super) fn standing_line(abilities: &Abilities, voice: Voice) -> String {
    if abilities.level == Level::Read {
        return "reads only; reports on or drops what it holds".into();
    }
    let mut actions = vec!["posts"];
    if abilities
        .rules
        .iter()
        .any(|rule| rule.rule == Rule::Review && rule.eligible)
    {
        actions.push("reviews");
    }
    if abilities
        .rules
        .iter()
        .any(|rule| matches!(rule.rule, Rule::Select | Rule::Finish) && rule.eligible)
    {
        actions.push("decides");
    }
    let follows = match (abilities.level, voice) {
        (Level::Ask, Voice::Person) => "waits for your yes before each task".to_owned(),
        (Level::Ask, Voice::Agent) => format!(
            "waits for {} before each task",
            owner_words(voice, Some(&abilities.name))
        ),
        _ => "takes tasks on its own".to_owned(),
    };
    format!("{}; {follows}", actions.join(", "))
}

fn task_state(task: &TaskView) -> String {
    let mut states = Vec::new();
    if task.closed {
        states.push("closed");
    }
    if task.selected.is_some() {
        states.push("selected");
    }
    if task.completed {
        states.push("completion declared");
    }
    if states.is_empty() {
        states.push("open");
    }
    states.join(", ")
}

fn task_label(task: TaskId, reader: &Reader) -> String {
    let id = reader.task_id(task);
    reader
        .tasks
        .iter()
        .find(|view| view.task == task)
        .and_then(|view| view.title.as_deref())
        .map(|title| format!("{} ({id})", safe(title)))
        .unwrap_or(id)
}

// Every command a view prints is complete: its reader runs it as printed, with
// only their own connection flags or environment. A view that cannot name a
// value prints a command that can, never a placeholder. The test
// `every_printed_command_parses_as_printed` feeds each one to the parser.
fn command_context(goal: &str) -> String {
    format!("locust {{operation}} --goal {goal}")
}

fn attempting_line(item: &WorkItem, reader: &Reader) -> String {
    let results = format!(
        "{} result{}",
        item.results,
        if item.results == 1 { "" } else { "s" }
    );
    if item.attempting.is_empty() {
        format!("  No other member is attempting it · {results}")
    } else {
        format!(
            "  Attempting: {} · {results}",
            item.attempting
                .iter()
                .map(|attempt| member_label(attempt.member, reader.members))
                .collect::<Vec<_>>()
                .join(", ")
        )
    }
}

fn pending(work: &PendingWork, goal: GoalId, reader: &Reader) -> Vec<String> {
    let mut lines = vec![format!("Observed revision {}", work.revision)];
    let goal_id = reader.goal_id(goal);
    let context = command_context(&goal_id);
    let owner = reader.owner();
    for item in &work.ask_first {
        lines.push(format!(
            "Waits for {owner}: {}",
            task_label(item.task, reader)
        ));
        lines.push(attempting_line(item, reader));
        lines.push(match (reader.voice, reader.local_name()) {
            (Voice::Person, Some(agent)) => format!(
                "  Allow this task: {}",
                allow_command(&goal_id, &reader.task_id(item.task), agent, false)
            ),
            (Voice::Agent, Some(agent)) => format!(
                "  {owner} can allow it: {}",
                allow_command(&goal_id, &reader.task_id(item.task), agent, false)
            ),
            (_, None) => "  Select an agent with --agent to see its task allowance command.".into(),
        });
    }
    for item in &work.to_start {
        lines.push(format!("Ready to start: {}", task_label(item.task, reader)));
        lines.push(attempting_line(item, reader));
        lines.push(format!(
            "  Participant action (requires its --session file): {} --task {}{}",
            context.replace("{operation}", "attempt start"),
            reader.task_id(item.task),
            item.offer
                .map(|offer| format!(" --offer {offer}"))
                .unwrap_or_default()
        ));
    }
    for claim in &work.claimed {
        lines.push(format!(
            "Claim held here: {} · attempt {} · generation {}",
            task_label(claim.task, reader),
            claim.attempt,
            claim.generation
        ));
    }
    for claim in &work.held_elsewhere {
        lines.push(format!(
            "Claim held by session {}: {} · attempt {} · generation {}",
            claim.instance,
            task_label(claim.task, reader),
            claim.attempt,
            claim.generation
        ));
        lines.push("  Resume that session, or take over after checking its work. A held claim does not prove a process is running.".into());
    }
    for item in &work.to_acknowledge {
        lines.push(format!(
            "Cancellation needs an outcome: {} · attempt {} · cancellation {}",
            task_label(item.task, reader),
            item.attempt,
            item.cancel
        ));
        lines.push("  Confirm whether the client stopped, completed, or has an uncertain outcome, then use cancel acknowledge in the claiming session.".into());
    }
    for item in &work.to_review {
        lines.push(format!(
            "Review needed: {} · {} · {} of {} approvals",
            item.subject,
            scope(item.context.scope),
            item.approvals,
            item.needed
        ));
        for verdict in &item.verdicts {
            lines.push(format!(
                "  {}: {} ({})",
                member_label(verdict.member, reader.members),
                match (verdict.opinion, verdict.approve) {
                    (true, true) => "opinion: approve",
                    (true, false) => "opinion: reject",
                    (false, true) => "approve",
                    (false, false) => "reject",
                },
                verdict.event
            ));
        }
        lines.push(format!(
            "  Read the exact submission: {} --event {}",
            context.replace("{operation}", "event show"),
            item.subject
        ));
        lines.push("  Record an approve or reject verdict with its reason using review record; review does not select or apply a change.".into());
    }
    for item in &work.deliveries {
        lines.push(format!(
            "Delivery {}: {} · {} · {}",
            item.effect,
            safe(&item.action),
            scope(item.context.scope),
            if !item.available {
                "no longer available"
            } else if item.acknowledged {
                "acknowledged"
            } else {
                "awaiting acknowledgment"
            }
        ));
        if item.available && !item.acknowledged {
            lines.push(format!(
                "  After reading it: {} --effect {}",
                context.replace("{operation}", "delivery acknowledge"),
                item.effect
            ));
        }
    }
    if let Some(news) = work.context_news
        && news.unacknowledged > 0
    {
        lines.push(format!(
            "Shared context: {} unacknowledged event versions ({} unavailable here)",
            news.unacknowledged, news.unavailable
        ));
        lines.push(format!(
            "  Read attributed findings and reviews: {} --view compact --limit 20",
            context.replace("{operation}", "context read")
        ));
    }
    if lines.len() == 1 {
        lines.push("No pending actions for this view.".into());
    } else {
        lines.push(
            "Participant commands use that participant's credential and execution session.".into(),
        );
    }
    lines
}

fn session(view: &SessionView, names: &[AgentView]) -> Vec<String> {
    let mut lines = vec![
        format!(
            "Session {} · {}",
            view.instance,
            label(view.principal, names)
        ),
        format!("  Client: {}", safe(&view.record.client)),
        format!(
            "  Adapter last reported: {} at {} ms since Unix epoch",
            tag(&view.record.state),
            view.updated_ms
        ),
        format!(
            "  Locust connection: {}",
            if view.attached {
                "attached"
            } else {
                "detached"
            }
        ),
    ];
    if let Some(client_session) = &view.record.client_session {
        lines.push(format!("  Client session: {}", safe(client_session)));
    }
    let capabilities = view.record.capabilities;
    let names = [
        ("tools", capabilities.tools),
        ("active delivery", capabilities.active_delivery),
        ("idle wake", capabilities.idle_wake),
        ("manual resume", capabilities.manual_resume),
        ("recovery", capabilities.recovery),
        ("confinement", capabilities.confinement),
    ]
    .into_iter()
    .filter_map(|(name, enabled)| enabled.then_some(name))
    .collect::<Vec<_>>();
    lines.push(format!(
        "  Reported capabilities: {}",
        if names.is_empty() {
            "none".into()
        } else {
            names.join(", ")
        }
    ));
    for claim in &view.claims {
        lines.push(format!(
            "  Claim: {} · attempt {} · generation {}",
            claim.task, claim.attempt, claim.generation
        ));
    }
    if view.record.state == SessionState::Blocked {
        lines.push("  Action: open the client and inspect its approval, sign-in, or input prompt. The daemon has no more specific reason.".into());
    } else if view.record.state == SessionState::Unknown {
        lines
            .push("  Action: inspect the client before resuming or taking over its claims.".into());
    }
    lines.push(
        "  A stored report or attached connection does not prove the client is currently running."
            .into(),
    );
    lines
}

/// One sentence for what waits for the person.
fn waiting_sentence(item: &WaitingForYou, goals: &[String]) -> String {
    let who = item
        .agent_name
        .as_deref()
        .map(safe)
        .unwrap_or_else(|| "An agent".into());
    let goal = item
        .title
        .as_deref()
        .map(quoted)
        .unwrap_or_else(|| short(&item.goal.to_string(), goals));
    let task_label = |task: &TaskId, task_title: &Option<String>| {
        task_title
            .as_deref()
            .map(quoted)
            .unwrap_or_else(|| short(&task.to_string(), &[]))
    };
    match &item.kind {
        WaitingKind::AllowTask { task, task_title } => format!(
            "{who} wants to take {} in {goal}",
            task_label(task, task_title)
        ),
        WaitingKind::SetAsk { task, task_title } => format!(
            "{who} wants to take {} in {goal} but is set to read",
            task_label(task, task_title)
        ),
    }
}

/// The heading of one goal in the status view: its title, its cut identifier
/// and who hosts it.
fn goal_heading(summary: &GoalSummary, goal_id: &str, voice: Voice) -> String {
    let host = if summary.abilities.hosted_here {
        format!(
            "host: {}",
            owner_words(voice, Some(&summary.abilities.name))
        )
    } else {
        match (summary.abilities.host, &summary.host_name) {
            (Some(_), Some(name)) => {
                format!("host: {}'s owner, on another computer", chosen_name(name))
            }
            (None, Some(name)) => format!(
                "host: on another computer · the ticket names {}; not confirmed until admission arrives",
                chosen_name(name)
            ),
            (_, None) => "host: on another computer".into(),
        }
    };
    format!(
        "{} ({goal_id}) · {host}{}",
        safe(summary.title.as_deref().unwrap_or("Title unavailable")),
        if summary.halted.is_some() {
            " · halted"
        } else {
            ""
        }
    )
}

/// One local agent under its goal: its name there with its local name, its
/// roles and level over its standing, or its membership over what to do. A
/// disconnected agent shows only that, over how to connect it again.
fn agent_lines(summary: &GoalSummary, agents: &[AgentView], voice: Voice) -> Vec<String> {
    let local = &summary.abilities.name;
    let label = if summary.name == *local {
        safe(local)
    } else {
        format!("{} ({})", chosen_name(&summary.name), safe(local))
    };
    if let Some(agent) = agents
        .iter()
        .find(|agent| agent.agent == summary.member && agent.revoked)
    {
        return vec![
            format!("  {label} · disconnected"),
            format!("      {}", super::disconnected_agent(agent)),
        ];
    }
    match summary.membership {
        Membership::Member => {
            let roles = if summary.abilities.roles.is_empty() {
                "member".to_owned()
            } else {
                summary
                    .abilities
                    .roles
                    .iter()
                    .map(|role| safe(role))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            vec![
                format!("  {label} · {roles} · {}", tag(&summary.abilities.level)),
                format!("      {}", standing_line(&summary.abilities, voice)),
            ]
        }
        other => {
            let mut lines = vec![format!("  {label} · {}", tag(&other))];
            if let Some(sentence) = membership_action(other, &owner_words(voice, Some(local))) {
                lines.push(format!("      {sentence}"));
            }
            lines
        }
    }
}

/// Mockup P5-1: what waits for the reader, each goal with its agents, the
/// agents in no goal, and the daemon.
fn status(status: &DaemonStatus, reader: &Reader) -> Vec<String> {
    let goal_ids: Vec<String> = status
        .goals
        .iter()
        .map(|summary| summary.goal.to_string())
        .collect();
    let owner = reader.owner();
    let mut lines = if status.waiting.is_empty() {
        vec![format!("Nothing is waiting for {owner}.")]
    } else {
        let mut lines = vec![format!("Waiting for {owner}")];
        for item in &status.waiting {
            lines.push(format!("  {}", waiting_sentence(item, &goal_ids)));
            lines.push(format!("    {}", item.command));
        }
        lines
    };
    let mut seen = Vec::new();
    for summary in &status.goals {
        if seen.contains(&summary.goal) {
            continue;
        }
        seen.push(summary.goal);
        let goal_id = short(&summary.goal.to_string(), &goal_ids);
        lines.push(String::new());
        lines.push(goal_heading(summary, &goal_id, reader.voice));
        if let Some(reason) = summary.halted {
            lines.push(format!("  {}", halt(reason)));
        }
        for entry in status
            .goals
            .iter()
            .filter(|entry| entry.goal == summary.goal)
        {
            lines.extend(agent_lines(entry, &status.agents, reader.voice));
        }
        if summary.invitations_open > 0 {
            let count = summary.invitations_open;
            lines.push(format!(
                "  {count} invitation{} open{}",
                if count == 1 { "" } else { "s" },
                match (count, summary.invitations_expire_ms) {
                    (1, Some(at)) => format!(", expires {}", expires_in(at, reader.now_ms)),
                    (_, Some(at)) =>
                        format!(", the latest expires {}", expires_in(at, reader.now_ms)),
                    (_, None) => String::new(),
                }
            ));
            lines.push(format!(
                "    locust --owner invitation revoke --goal {goal_id} --all"
            ));
        }
    }
    let idle: Vec<String> = status
        .agents
        .iter()
        .map(|agent| {
            if status.goals.iter().any(|entry| entry.member == agent.agent) {
                String::new()
            } else if agent.revoked {
                super::disconnected_agent(agent)
            } else {
                format!("{} is connected and in no goal.", safe(&agent.name))
            }
        })
        .filter(|line| !line.is_empty())
        .collect();
    if !idle.is_empty() {
        lines.push(String::new());
        lines.extend(idle);
    }
    lines.push(format!(
        "Daemon {}{}",
        safe(&status.daemon_version),
        status
            .endpoint
            .map(|endpoint| format!(
                " · endpoint {}",
                endpoint.to_string().chars().take(8).collect::<String>()
            ))
            .unwrap_or_default()
    ));
    lines
}

pub(super) fn goal_status(
    view: &GoalStatus,
    formation: Option<&Formation>,
    reader: &Reader,
) -> String {
    let goal_id = reader.goal_id(view.goal);
    let mut lines = vec![
        format!(
            "{} ({})",
            safe(view.title.as_deref().unwrap_or("Title unavailable")),
            view.goal
        ),
        host_line(view, reader),
    ];
    if let Some(reason) = view.halted {
        lines.push(halt(reason).into());
    }
    for item in &view.scope_halts {
        lines.push(format!(
            "{}: {}",
            scope(item.context.scope),
            halt(item.reason)
        ));
    }
    for member in &view.members {
        lines.push(format!(
            "Member: {} · {} · endpoint {}",
            member_label(member.member, &view.members),
            if member.local { "local" } else { "remote" },
            member
                .endpoint
                .to_string()
                .chars()
                .take(8)
                .collect::<String>()
        ));
        let roles = view
            .roles
            .iter()
            .filter(|(_, holders)| holders.contains(&member.member))
            .map(|(role, _)| safe(role))
            .collect::<Vec<_>>();
        if !roles.is_empty() {
            lines
                .last_mut()
                .expect("member line")
                .push_str(&format!(" · {}", roles.join(", ")));
        }
    }
    if !view.roles.is_empty() {
        lines.push(format!(
            "Roles: {}",
            view.roles
                .iter()
                .map(|(role, holders)| format!(
                    "{}{} {}",
                    safe(role),
                    if formation.is_some_and(|formation| !formation.roles.contains_key(role)) {
                        " (earlier rules)"
                    } else {
                        ""
                    },
                    holders
                        .iter()
                        .map(|holder| member_label(*holder, &view.members))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join(" · ")
        ));
    }
    if let Some(formation) = formation
        && let Some(role) = super::roles::counting_role(formation)
    {
        let missing = super::roles::missing_reviewers(formation, &role, &view.roles).unwrap_or(0);
        if missing > 0 {
            // Members already in the goal who lack the role come first: giving
            // it to them is the step that makes results count. Only the host
            // adds or invites members; on another computer the sentence must
            // not point the reader at a step it cannot take.
            let unheld: Vec<_> = if view.hosted_here {
                view.members
                    .iter()
                    .filter(|member| {
                        !view
                            .roles
                            .get(&role)
                            .is_some_and(|holders| holders.contains(&member.member))
                    })
                    .collect()
            } else {
                Vec::new()
            };
            let tail = match (view.hosted_here, reader.voice) {
                (true, _) if unheld.len() >= missing => String::new(),
                (true, Voice::Person) => {
                    format!(" members you add or invite become {}s.", safe(&role))
                }
                (true, Voice::Agent) => format!(
                    " members {} adds or invites become {}s.",
                    reader.owner(),
                    safe(&role)
                ),
                (false, _) => " the host adds or invites them.".to_owned(),
            };
            lines.push(format!(
                "{missing} more {}{} needed{}{tail}",
                safe(&role),
                if missing == 1 { " is" } else { "s are" },
                if tail.is_empty() { "." } else { ";" }
            ));
            for member in unheld {
                let key = super::selectors::member_key_prefix(&view.members, member.member);
                lines.push(match reader.voice {
                    Voice::Person => super::roles::role_command_line(
                        "Give the role",
                        "give",
                        &goal_id,
                        &key,
                        &role,
                    ),
                    Voice::Agent => format!(
                        "{} can give it to {}.",
                        reader.owner(),
                        member_label(member.member, &view.members)
                    ),
                });
            }
        }
    }
    if let Some(rules) = view.current_rules {
        lines.push(format!("Rules revision: {rules}"));
    }
    if let Some(workspace) = &view.workspace {
        lines.push(format!("Shared workspace: {}", tag(&workspace.authority)));
        if let Some(head) = &workspace.head {
            lines.push(format!("Shared revision: {}", head.revision));
        }
    }
    for abilities in &view.abilities {
        lines.push(format!(
            "{} · level {} · {}",
            safe(&abilities.name),
            tag(&abilities.level),
            standing_line(abilities, reader.voice)
        ));
        // The command goes on its own line under the sentence: a title is
        // another member's text and must not share a line with what the
        // reader is meant to copy.
        for wanted in &abilities.wanted_tasks {
            let title = wanted
                .title
                .as_deref()
                .map(quoted)
                .unwrap_or_else(|| "this task".into());
            // An allowance lowers the bar to ask, so at read only a level helps.
            if abilities.level == Level::Read {
                lines.push(format!(
                    "  Asked to take {title}, but at read it only reads."
                ));
                lines.push(format!(
                    "    {}",
                    level_command(&goal_id, &abilities.name, "ask")
                ));
            } else {
                lines.push(format!("  Asked to take {title}:"));
                lines.push(format!(
                    "    {}",
                    allow_command(
                        &goal_id,
                        &reader.task_id(wanted.task),
                        &abilities.name,
                        false
                    )
                ));
            }
        }
    }
    for stalled in &view.stalled {
        lines.push(format!(
            "Step {} stalled for {}: {}",
            stalled.effect,
            signer(
                stalled.runner == view.governance,
                stalled.runner,
                reader.names
            ),
            tag(&stalled.reason)
        ));
    }
    for peer in &view.peers {
        lines.push(format!(
            "Peer {}: {} · last successful sync {}",
            peer.endpoint,
            if peer.connected {
                "connected"
            } else {
                "disconnected"
            },
            peer.last_sync_ms
                .map(|at| format!("{at} ms since Unix epoch"))
                .unwrap_or_else(|| "not observed".into())
        ));
    }
    lines.push(
        "Peer connectivity describes this daemon's observation, not a remote process's activity."
            .into(),
    );
    lines.join("\n")
}

pub(super) fn render(response: &Response, goal: Option<GoalId>, reader: &Reader) -> Option<String> {
    let names = reader.names;
    let by_owner = match reader.voice {
        Voice::Person => "by you",
        Voice::Agent => "by your owner",
    };
    let lines = match response {
        Response::Status(view) => status(view, reader),
        Response::GoalStatus(view) => return Some(goal_status(view, None, reader)),
        Response::Board(tasks) => {
            if tasks.is_empty() { vec!["No tasks in this goal.".into()] } else {
                let board = Reader { tasks, ..*reader };
                let mut lines = vec![format!("{} tasks", tasks.len())];
                for task in tasks {
                    lines.push(format!("{} · {}", task_label(task.task, &board), task_state(task)));
                    lines.push(format!("  By {} · {} attempts · {} contributions", signer(task.by_host, task.creator, names), task.attempts.len(), task.contributions.len()));
                }
                lines.push("Use pending for start, review and acknowledgment actions.".into());
                lines
            }
        }
        Response::Task(task) => {
            let mut lines = vec![format!("{} · {}", task.view.task, task_state(&task.view)), format!("Created by {}", signer(task.view.by_host, task.view.creator, names)), format!("Text: {}", task.text.as_deref().map(safe).unwrap_or_else(|| "not held locally".into())), format!("Current round: {}", task.view.context.round)];
            if let Some(parent) = task.parent { lines.push(format!("Parent: {parent}")); }
            if let Some(task_type) = &task.task_type { lines.push(format!("Task type: {}", safe(task_type))); }
            for (name, input) in &task.inputs { lines.push(format!("Input {}: {input}", safe(name))); }
            for attempt in &task.view.attempts { lines.push(format!("Attempt: {attempt}")); }
            for contribution in &task.view.contributions { lines.push(format!("Contribution: {contribution}")); }
            if let Some(selected) = task.view.selected { lines.push(format!("Selected: {selected}")); }
            let mut rules: serde_json::Value = serde_json::from_str(&task.effective_rules_json).expect("effective rules JSON");
            if let Some(rules) = rules.as_object_mut() {
                // The creator is already shown above with its human label.
                rules.remove("creator");
            }
            lines.push(format!("Effective rules: {}", safe(&rules.to_string())));
            lines
        }
        Response::Pending(work) => pending(work, goal?, reader),
        Response::Waited(WaitOutcome::Work(work)) => pending(work, goal?, reader),
        Response::Waited(WaitOutcome::NoEvent) => vec!["No observed change before the requested timeout.".into()],
        Response::Waited(WaitOutcome::Disconnected) => vec!["No observed change before the requested timeout. No peer of this goal is currently reachable.".into()],
        Response::Contributions(contributions) => {
            let mut lines = Vec::new();
            for contribution in contributions {
                lines.push(format!("Contribution {} · {} · {}", contribution.contribution, label(contribution.author, names), scope(contribution.context.scope)));
                lines.push(format!("  Review: {} · Selection: {}", if contribution.approved { "approved" } else { "not approved" }, if contribution.selected { "selected" } else { "not selected" }));
                lines.push(format!("  {}", contribution.text.as_deref().map(safe).unwrap_or_else(|| "Text is not held locally.".into())));
                for source in &contribution.sources { lines.push(format!("  Declared source: {source}")); }
                for artifact in &contribution.artifacts { lines.push(format!("  Artifact: {artifact}")); }
                for evidence in &contribution.evidence { lines.push(format!("  Evidence: {evidence}")); }
            }
            if lines.is_empty() { lines.push("No contributions in this view.".into()); }
            lines
        }
        Response::ContributionInspected(inspected) => {
            let event = &inspected.contribution;
            let mut lines = vec![format!("Contribution {} · {}", event.view.event, tag(&event.view.standing)), format!("Author: {}", signer(event.view.by_host, event.view.author, names))];
            if event.view.by_owner { lines.push(by_owner.into()); }
            if let Some(text) = &event.text { lines.push(format!("Summary: {}", safe(text))); }
            else if event.payload.is_some() { lines.push("Summary text is not held locally.".into()); }
            if let Some(task) = event.task { lines.push(format!("Task: {task}")); }
            for (kind, reference) in inspected.attempt.iter().map(|item| ("Attempt", item))
                .chain(inspected.task_round.iter().map(|item| ("Task round", item)))
                .chain(inspected.declared_sources.iter().map(|item| ("Declared source", item))) {
                match &reference.detail {
                    Some(detail) => {
                        lines.push(format!("{kind}: {} · {} · {} · {}", reference.event, safe(&detail.view.kind), signer(detail.view.by_host, detail.view.author, names), tag(&detail.view.standing)));
                        if let Some(text) = &detail.text { lines.push(format!("  {}", safe(text))); }
                        else if detail.payload.is_some() { lines.push("  Text is not held locally.".into()); }
                        for content in &detail.content { lines.push(format!("  Content {}: {}", content.hash, tag(&content.state))); }
                    }
                    None => lines.push(format!("{kind}: {} · event is not held locally", reference.event)),
                }
            }
            if inspected.declared_sources.is_empty() { lines.push("No sources declared.".into()); }
            else { lines.push("Sources are signed declarations by the author; they do not prove use or approval.".into()); }
            lines
        }
        Response::Session(view) => session(view, names),
        Response::Sessions(views) => {
            let mut lines = views.iter().flat_map(|view| session(view, names)).collect::<Vec<_>>();
            if lines.is_empty() { lines.push("No adapter session reports.".into()); }
            lines
        }
        Response::Event(event) => {
            let mut lines = vec![format!("Event {} · {} · {}", event.view.event, safe(&event.view.kind), tag(&event.view.standing)), format!("Author: {}", signer(event.view.by_host, event.view.author, names))];
            if event.view.by_owner { lines.push(by_owner.into()); }
            match &event.body {
                Body::Genesis(genesis) => lines.push(format!("Goal created. Host's agent: {}. Definition: {}", label(genesis.host, names), genesis.definition)),
                Body::ReviewRecorded { subject, verdict, .. } => lines.push(format!("Review of {subject}: {}", tag(verdict))),
                Body::AttemptReported { attempt, status } => lines.push(format!("Attempt {attempt}: {}", tag(status))),
                Body::CheckAttested { subject, name, passed, .. } => lines.push(format!("Check {} for {subject}: {}", safe(name), if *passed { "passed" } else { "failed" })),
                _ => lines.push(format!("Record: {}", safe(&serde_json::to_string(&event.body).expect("event serializes")))),
            }
            if let Some(task) = event.task { lines.push(format!("Task: {task}")); }
            if let Some(text) = &event.text { lines.push(format!("Text: {}", safe(text))); }
            else if event.payload.is_some() { lines.push("Text is not held locally.".into()); }
            for content in &event.content { lines.push(format!("Content {}: {}", content.hash, tag(&content.state))); }
            lines
        }
        Response::Events(events) => {
            let mut lines = events.iter().map(|event| format!("{} · {} · {} · {}{}", event.event, safe(&event.kind), signer(event.by_host, event.author, names), tag(&event.standing), if event.by_owner { format!(" · {by_owner}") } else { String::new() })).collect::<Vec<_>>();
            if lines.is_empty() { lines.push("No events in this page.".into()); }
            lines
        }
        _ => return None,
    };
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::{Ability, AgentView};

    fn person() -> Reader<'static> {
        Reader::new(Voice::Person, None, &[])
    }

    fn agent(principal: PublicKey) -> Reader<'static> {
        Reader::new(Voice::Agent, Some(principal), &[])
    }

    #[test]
    fn standing_line_follows_the_level_and_eligible_rows() {
        let mut abilities = Abilities {
            goal: GoalId([1; 32]),
            agent: PublicKey([2; 32]),
            name: "worker".into(),
            membership: Some(Membership::Member),
            level: Level::Read,
            host: Some(PublicKey([2; 32])),
            hosted_here: true,
            roles: vec![],
            rules: vec![],
            allowed_tasks: vec![],
            wanted_tasks: vec![],
            claims: vec![],
        };
        assert_eq!(
            standing_line(&abilities, Voice::Person),
            "reads only; reports on or drops what it holds"
        );
        abilities.level = Level::Ask;
        assert_eq!(
            standing_line(&abilities, Voice::Person),
            "posts; waits for your yes before each task"
        );
        assert_eq!(
            standing_line(&abilities, Voice::Agent),
            "posts; waits for worker's owner before each task"
        );
        abilities.rules.push(Ability {
            rule: Rule::Review,
            qualifies: Selector::Members,
            except_author: false,
            eligible: true,
            needs: Level::Ask,
            allowed: true,
        });
        abilities.rules.push(Ability {
            rule: Rule::Select,
            qualifies: Selector::Nobody,
            except_author: false,
            eligible: false,
            needs: Level::Ask,
            allowed: false,
        });
        assert_eq!(
            standing_line(&abilities, Voice::Person),
            "posts, reviews; waits for your yes before each task"
        );
        abilities.level = Level::Auto;
        abilities.rules[1].eligible = true;
        assert_eq!(
            standing_line(&abilities, Voice::Agent),
            "posts, reviews, decides; takes tasks on its own"
        );
    }

    #[test]
    fn an_event_the_person_signed_reads_by_you_or_by_your_owner() {
        use locust_proto::api::{EventView, Standing};
        use locust_proto::id::EventId;
        let agent_key = PublicKey([2; 32]);
        let view = EventView {
            position: Some(1),
            event: EventId([3; 32]),
            author: agent_key,
            kind: "task_opened".into(),
            at_ms: 1,
            standing: Standing::Effective,
            by_owner: true,
            by_host: false,
        };
        let response = Response::Events(vec![view]);
        assert!(
            render(&response, None, &person())
                .unwrap()
                .contains("by you")
        );
        assert!(
            render(&response, None, &agent(agent_key))
                .unwrap()
                .contains("by your owner")
        );
    }

    /// The goal's signing key, its host's agent and a goal whose first record
    /// is held here, with the responses that name a record's signer.
    fn hosted(governance: PublicKey, hosted_here: bool) -> Vec<Response> {
        use locust_proto::api::{EventView, MemberView, Stall, Stalled, Standing};
        use locust_proto::event::{Effect, EffectAction, Trigger};
        use locust_proto::id::{EffectId, EndpointId, EventId};
        let goal = GoalId([1; 32]);
        let agent = PublicKey([2; 32]);
        let event = EventId([3; 32]);
        let context = locust_proto::event::Context {
            scope: Scope::Goal,
            round: event,
        };
        let effect = Effect {
            context,
            transition: "start".into(),
            trigger: Trigger::Stage {
                rules: event,
                stage: "one".into(),
            },
            target_slot: "stage".into(),
            action: EffectAction::Offer {
                context,
                recipient: agent,
            },
            evidence: vec![],
        };
        let signed = |author: PublicKey| EventView {
            position: Some(1),
            event,
            author,
            kind: "effect_materialized".into(),
            at_ms: 1,
            standing: Standing::Effective,
            by_owner: false,
            by_host: author == governance,
        };
        let task = TaskView {
            task: TaskId::Derived(EffectId([6; 32])),
            context,
            creator: governance,
            by_host: true,
            title: Some("Stage one".into()),
            attempts: vec![],
            contributions: vec![],
            completed: false,
            selected: None,
            closed: false,
        };
        let status = GoalStatus {
            guard: vec![],
            restored: None,
            host_name: Some("Host".into()),
            roles: Default::default(),
            deciding: Default::default(),
            acting_alone: Default::default(),

            goal,
            title: Some("Parser cleanup".into()),
            governance,
            hosted_here,
            host: Some(agent),
            governance_head: Some(event),
            current_rules: Some(event),
            scope_halts: vec![],
            members: vec![MemberView {
                name: "maple".into(),

                member: agent,
                endpoint: EndpointId([3; 32]),
                local: true,
                admitted: 0,
            }],
            halted: Some(Halt::AuthorityConflict),
            workspace: None,
            abilities: vec![],
            stalled: vec![Stalled {
                effect: EffectId([6; 32]),
                runner: governance,
                reason: Stall::Halted,
            }],
            peers: vec![],
        };
        vec![
            Response::Events(vec![signed(governance), signed(agent)]),
            Response::Event(Box::new(locust_proto::api::EventDetail {
                view: signed(governance),
                anchor: Some(event),
                body: Body::EffectMaterialized { effect },
                payload: None,
                text: None,
                task: None,
                content: vec![],
            })),
            Response::Board(vec![task.clone()]),
            Response::Task(locust_proto::api::TaskDetail {
                view: task,
                text: Some("Stage one".into()),
                inputs: Default::default(),
                parent: None,
                task_type: None,
                effective_rules_json:
                    serde_json::json!({"creator": governance, "work": {}, "decisions": {}})
                        .to_string(),
            }),
            Response::GoalStatus(status),
        ]
    }

    #[test]
    fn a_record_signed_by_the_governance_key_prints_host() {
        let governance = PublicKey([9; 32]);
        let agent_key = PublicKey([2; 32]);
        let names = [AgentView {
            agent: agent_key,
            name: "maple".into(),
            author_only: false,
            revoked: false,
        }];
        let reader = Reader::new(Voice::Person, None, &names);
        let rendered: Vec<_> = hosted(governance, true)
            .iter()
            .map(|response| render(response, Some(GoalId([1; 32])), &reader).unwrap())
            .collect();
        let [events, event, board, task, status] = rendered.as_slice() else {
            unreachable!("five responses")
        };
        assert!(events.contains(" · host · "), "{events}");
        assert!(events.contains(&format!("maple ({agent_key})")), "{events}");
        assert!(event.contains("Author: host"), "{event}");
        assert!(board.contains("By host ·"), "{board}");
        assert!(task.contains("Created by host"), "{task}");
        assert!(status.contains("stalled for host:"), "{status}");
        assert!(status.contains("Host: you"), "{status}");
        let elsewhere = render(
            hosted(governance, false).last().unwrap(),
            Some(GoalId([1; 32])),
            &reader,
        )
        .unwrap();
        assert!(
            elsewhere.contains("Host: on another computer · maple (02020202)"),
            "{elsewhere}"
        );
    }

    #[test]
    fn rendered_text_never_names_the_governance_key() {
        let governance = PublicKey([9; 32]);
        for hosted_here in [true, false] {
            for response in hosted(governance, hosted_here) {
                let text = render(&response, Some(GoalId([1; 32])), &person()).unwrap();
                assert!(!text.contains(&governance.to_string()), "{text}");
                assert!(!text.to_lowercase().contains("governance key"), "{text}");
                if let Response::Event(mut detail) = response {
                    detail.body = Body::Genesis(locust_proto::event::Genesis {
                        governance,
                        host: PublicKey([2; 32]),
                        definition: locust_proto::id::DefinitionHash([4; 32]),
                        salt: [5; 16],
                    });
                    let text =
                        render(&Response::Event(detail), Some(GoalId([1; 32])), &person()).unwrap();
                    assert!(!text.contains(&governance.to_string()), "{text}");
                    assert!(text.contains("Goal created."), "{text}");
                }
            }
        }
        let response = hosted(governance, true).pop().unwrap();
        let text = render(&response, Some(GoalId([1; 32])), &agent(PublicKey([2; 32]))).unwrap();
        assert!(text.contains("Host: your owner"), "{text}");
        assert!(!text.contains("Host: you ·"), "{text}");
    }

    #[test]
    fn a_ticket_host_name_reads_as_unconfirmed_until_the_record_is_held() {
        let Response::GoalStatus(mut view) = hosted(PublicKey([9; 32]), false).pop().unwrap()
        else {
            panic!("goal status fixture")
        };
        view.host = None;
        view.members.clear();
        view.host_name = Some("Harbor · lead".into());
        let text = goal_status(&view, None, &person());
        assert!(
            text.contains(
                "Host: on another computer · the ticket names \"Harbor · lead\"; not confirmed until admission arrives"
            ),
            "{text}"
        );
        view.host_name = None;
        let text = goal_status(&view, None, &person());
        assert!(
            text.contains("Host: on another computer · name has not arrived"),
            "{text}"
        );
    }

    #[test]
    fn dates_expiry_and_completion_words_are_specific() {
        assert_eq!(utc(0), "1970-01-01 00:00 UTC");
        assert_eq!(utc(1_784_070_000_000), "2026-07-14 23:00 UTC");
        assert_eq!(expires_in(0, 0), "expired");
        assert_eq!(expires_in(3_600_001, 0), "in 2 hours");
        assert_eq!(expires_in(86_400_000, 0), "in 1 day");
        assert_eq!(
            counts_when(&CompletionRule::Reviews {
                by: Selector::Members,
                count: 2,
                exclude_author: true,
            }),
            "A result counts when it has 2 approvals, not the author's."
        );
    }

    #[test]
    fn an_or_inside_an_and_is_bracketed() {
        let rule = CompletionRule::All {
            rules: vec![
                CompletionRule::Any {
                    rules: vec![
                        CompletionRule::Reviews {
                            by: Selector::Members,
                            count: 1,
                            exclude_author: true,
                        },
                        CompletionRule::Contribution {
                            by: Selector::OnlyMember,
                        },
                    ],
                },
                CompletionRule::Check {
                    name: "tests".into(),
                    by: Selector::Members,
                },
            ],
        };
        assert_eq!(
            counts_when(&rule),
            "A result counts when (it has 1 approval, not the author's, or the goal's only member posts it) and the check \"tests\" is reported as passed."
        );
    }

    #[test]
    fn goal_status_names_the_host_members_and_roles_and_counts_missing_reviewers() {
        let Response::GoalStatus(mut view) = hosted(PublicKey([9; 32]), false).pop().unwrap()
        else {
            panic!("goal status fixture")
        };
        view.halted = None;
        view.stalled.clear();
        view.members[0].name = "Harbor".into();
        view.host_name = Some("Harbor".into());
        view.roles
            .insert("reviewer".into(), vec![view.host.unwrap()]);
        view.roles.insert("lead".into(), vec![view.host.unwrap()]);
        let panel = locust_proto::organization::presets()
            .into_iter()
            .find(|preset| preset.name == "review-panel")
            .unwrap()
            .formation;
        let text = goal_status(&view, Some(&panel), &person());
        assert!(
            text.contains("Host: on another computer · Harbor (02020202)"),
            "{text}"
        );
        assert!(
            text.contains("Member: Harbor (02020202) · local · endpoint 03030303 · lead, reviewer"),
            "{text}"
        );
        assert!(
            text.contains(
                "Roles: lead (earlier rules) Harbor (02020202) · reviewer Harbor (02020202)"
            ),
            "{text}"
        );
        assert!(
            text.contains("2 more reviewers are needed; the host adds or invites them."),
            "{text}"
        );
        view.hosted_here = true;
        let text = goal_status(&view, Some(&panel), &person());
        assert!(
            text.contains(
                "2 more reviewers are needed; members you add or invite become reviewers."
            ),
            "{text}"
        );
        let text = goal_status(&view, Some(&panel), &agent(view.host.unwrap()));
        assert!(
            text.contains("members your owner adds or invites become reviewers."),
            "{text}"
        );
        // Members already in the goal who lack the role come first; the
        // add-or-invite tail stays only while they do not cover the shortfall.
        view.members.push(MemberView {
            member: PublicKey([5; 32]),
            name: "Maple".into(),
            endpoint: locust_proto::id::EndpointId([3; 32]),
            local: false,
            admitted: 1,
        });
        let text = goal_status(&view, Some(&panel), &person());
        assert!(
            text.contains(
                "2 more reviewers are needed; members you add or invite become reviewers.\nGive the role: locust --owner role give --goal 01010101 --member 05050505 reviewer\n"
            ),
            "{text}"
        );
        view.members.push(MemberView {
            member: PublicKey([6; 32]),
            name: "Juniper".into(),
            endpoint: locust_proto::id::EndpointId([3; 32]),
            local: false,
            admitted: 2,
        });
        let text = goal_status(&view, Some(&panel), &person());
        assert!(
            text.contains(
                "2 more reviewers are needed.\nGive the role: locust --owner role give --goal 01010101 --member 05050505 reviewer\nGive the role: locust --owner role give --goal 01010101 --member 06060606 reviewer\n"
            ),
            "{text}"
        );
        let text = goal_status(&view, Some(&panel), &agent(view.host.unwrap()));
        assert!(
            text.contains(
                "2 more reviewers are needed.\nyour owner can give it to Maple (05050505).\nyour owner can give it to Juniper (06060606).\n"
            ),
            "{text}"
        );
        view.hosted_here = false;
        let text = goal_status(&view, Some(&panel), &person());
        assert!(
            text.contains("2 more reviewers are needed; the host adds or invites them.\n"),
            "{text}"
        );
        assert!(!text.contains("Give the role"), "{text}");
        view.hosted_here = true;
        // A role named inside an any-of choice counts the holders of every
        // role the choice names.
        let mut either = panel.clone();
        either.roles.insert(
            "senior".into(),
            locust_proto::organization::Role {
                description: "Senior reviewers.".into(),
            },
        );
        either.decisions.completion = locust_proto::organization::CompletionRule::Reviews {
            by: locust_proto::organization::Selector::Any {
                selectors: vec![
                    locust_proto::organization::Selector::Role {
                        name: "reviewer".into(),
                    },
                    locust_proto::organization::Selector::Role {
                        name: "senior".into(),
                    },
                ],
            },
            count: 2,
            exclude_author: true,
        };
        view.roles.insert("senior".into(), vec![PublicKey([5; 32])]);
        let text = goal_status(&view, Some(&either), &person());
        assert!(
            text.contains(
                "1 more reviewer is needed.\nGive the role: locust --owner role give --goal 01010101 --member 05050505 reviewer\nGive the role: locust --owner role give --goal 01010101 --member 06060606 reviewer\n"
            ),
            "{text}"
        );
        view.members.truncate(1);
        view.roles.remove("senior");
        let peer = locust_proto::organization::presets()
            .into_iter()
            .find(|preset| preset.name == "peer-review")
            .unwrap()
            .formation;
        view.roles.clear();
        let text = goal_status(&view, Some(&peer), &person());
        assert!(!text.contains("Roles:"), "{text}");
        assert!(!text.contains("more reviewers"), "{text}");
        assert_eq!(
            counts_when(&peer.decisions.completion),
            "A result counts when it has 1 approval, not the author's, or the goal's only member posts it."
        );
    }

    #[test]
    fn a_name_never_replaces_its_identity() {
        let key = PublicKey([1; 32]);
        let names = [AgentView {
            agent: key,
            name: "worker".into(),
            author_only: false,
            revoked: false,
        }];
        assert_eq!(label(key, &names), format!("worker ({key})"));
        let mut members = [MemberView {
            member: key,
            name: "Maple".into(),
            endpoint: locust_proto::id::EndpointId([2; 32]),
            local: false,
            admitted: 0,
        }];
        assert_eq!(member_label(key, &members), "Maple (01010101)");
        assert_eq!(
            member_label_noting(key, &members, "the host's agent"),
            "Maple (01010101, the host's agent)"
        );
        // A chosen name cannot imitate a line's separators.
        members[0].name = "Zed (0a0b0c0d) · lead \"Maple\"".into();
        assert_eq!(
            member_label(key, &members),
            "\"Zed (0a0b0c0d) · lead \\\"Maple\\\"\" (01010101)"
        );
        assert_eq!(chosen_name("Ana Maria"), "Ana Maria");
        assert_eq!(chosen_name(" Ana"), "\" Ana\"");
        assert_eq!(chosen_name("Zed\u{202e}"), "\"Zed\\u{202e}\"");
        // Two members whose keys share eight characters get longer prefixes,
        // so a look-alike key under the same name never prints the same label.
        let mut near = key.0;
        near[4] = 2;
        let members = [
            MemberView {
                member: key,
                name: "Maple".into(),
                endpoint: locust_proto::id::EndpointId([2; 32]),
                local: false,
                admitted: 0,
            },
            MemberView {
                member: PublicKey(near),
                name: "Maple".into(),
                endpoint: locust_proto::id::EndpointId([2; 32]),
                local: false,
                admitted: 1,
            },
        ];
        assert_eq!(member_label(key, &members), "Maple (0101010101)");
        assert_eq!(
            member_label(PublicKey(near), &members),
            "Maple (0101010102)"
        );
    }

    /// One of everything a view can ask its reader to do next.
    fn work() -> PendingWork {
        use locust_proto::api::{
            Attempting, CancelItem, Claim, ContextNews, DeliveryItem, ReviewItem, WorkItem,
        };
        use locust_proto::id::{EffectId, EventId, InstanceId};
        let event = EventId([4; 32]);
        let task = TaskId::Authored(event);
        let context = locust_proto::event::Context {
            scope: Scope::Task(task),
            round: event,
        };
        let claim = Claim {
            goal: GoalId([1; 32]),
            task,
            attempt: event,
            instance: InstanceId([5; 16]),
            generation: 1,
        };
        PendingWork {
            workspace: None,
            revision: 7,
            context_news: Some(ContextNews {
                unacknowledged: 2,
                unavailable: 0,
            }),
            ask_first: vec![WorkItem {
                unattended: false,
                task,
                offer: None,
                attempting: vec![],
                results: 0,
            }],
            to_start: vec![
                WorkItem {
                    unattended: false,
                    task,
                    offer: None,
                    attempting: vec![Attempting {
                        member: PublicKey([7; 32]),
                        status: None,
                    }],
                    results: 1,
                },
                WorkItem {
                    unattended: false,
                    task: TaskId::Derived(EffectId([6; 32])),
                    offer: Some(event),
                    attempting: vec![],
                    results: 0,
                },
            ],
            claimed: vec![claim],
            held_elsewhere: vec![claim],
            to_acknowledge: vec![CancelItem {
                task,
                attempt: event,
                cancel: event,
                generation: Some(1),
            }],
            to_review: vec![ReviewItem {
                subject: event,
                context,
                approvals: 0,
                needed: 1,
                verdicts: vec![],
            }],
            deliveries: vec![DeliveryItem {
                effect: EffectId([6; 32]),
                context,
                acknowledged: false,
                received: true,
                available: true,
                action: "notify".into(),
            }],
        }
    }

    fn names() -> Vec<AgentView> {
        vec![
            AgentView {
                agent: PublicKey([2; 32]),
                name: "codex-maple-1a2b3c4d".into(),
                author_only: false,
                revoked: false,
            },
            AgentView {
                agent: PublicKey([3; 32]),
                name: "claude-juniper-77aa0c52".into(),
                author_only: false,
                revoked: false,
            },
            AgentView {
                agent: PublicKey([4; 32]),
                name: "codex-birch-5e6f7a8b".into(),
                author_only: false,
                revoked: false,
            },
        ]
    }

    fn id<T: std::str::FromStr>(prefix: &str) -> T
    where
        T::Err: std::fmt::Debug,
    {
        format!("{prefix}{}", "0".repeat(64 - prefix.len()))
            .parse()
            .unwrap()
    }

    enum Host {
        Here(&'static str),
        Elsewhere(&'static str),
    }

    fn summary(
        (goal, title): (GoalId, &str),
        agent: usize,
        name: &str,
        membership: Membership,
        level: Level,
        roles: &[&str],
        host: Host,
    ) -> GoalSummary {
        let names = names();
        let agent = &names[agent];
        let (hosted_here, host_name) = match host {
            Host::Here(name) => (true, name),
            Host::Elsewhere(name) => (false, name),
        };
        let mut rules = vec![];
        if roles.contains(&"reviewer") {
            rules.push(Ability {
                rule: Rule::Review,
                qualifies: Selector::Role {
                    name: "reviewer".into(),
                },
                except_author: true,
                eligible: true,
                needs: Level::Ask,
                allowed: true,
            });
        }
        if roles.contains(&"lead") {
            rules.push(Ability {
                rule: Rule::Select,
                qualifies: Selector::Role {
                    name: "lead".into(),
                },
                except_author: false,
                eligible: true,
                needs: Level::Ask,
                allowed: true,
            });
        }
        GoalSummary {
            guard: vec![],
            restored: None,
            goal,
            title: Some(title.into()),
            member: agent.agent,
            name: name.into(),
            membership,
            host_name: Some(host_name.into()),
            invitations_open: 0,
            invitations_expire_ms: None,
            halted: None,
            abilities: Abilities {
                goal,
                agent: agent.agent,
                name: agent.name.clone(),
                membership: Some(membership),
                level,
                host: Some(PublicKey([8; 32])),
                hosted_here,
                roles: roles.iter().map(|role| (*role).into()).collect(),
                rules,
                allowed_tasks: vec![],
                wanted_tasks: vec![],
                claims: vec![],
            },
        }
    }

    /// Mockup P5-1.
    fn p5_status() -> DaemonStatus {
        let parser: GoalId = id("c01d55aa");
        let search: GoalId = id("7f3a9c1e");
        let task = TaskId::Authored(id("4b2d8e01"));
        const DAY: u64 = 86_400_000;
        let mut hosted = summary(
            (parser, "Parser cleanup"),
            0,
            "Maple",
            Membership::Member,
            Level::Auto,
            &["lead", "reviewer"],
            Host::Here("Maple"),
        );
        hosted.invitations_open = 1;
        hosted.invitations_expire_ms = Some(6 * DAY + 3_600_000);
        DaemonStatus {
            daemon_version: "0.1.0".into(),
            endpoint: Some(id("5c0e77aa")),
            agents: names(),
            waiting: vec![WaitingForYou {
                goal: search,
                title: Some("Static site search".into()),
                agent: Some(PublicKey([2; 32])),
                agent_name: Some("Maple".into()),
                kind: WaitingKind::AllowTask {
                    task,
                    task_title: Some("Fix the parser".into()),
                },
                command: allow_command("7f3a9c1e", "task:4b2d8e01", "codex-maple-1a2b3c4d", false),
            }],
            goals: vec![
                hosted,
                summary(
                    (parser, "Parser cleanup"),
                    1,
                    "Juniper",
                    Membership::Member,
                    Level::Read,
                    &[],
                    Host::Here("Maple"),
                ),
                summary(
                    (search, "Static site search"),
                    0,
                    "Maple",
                    Membership::Member,
                    Level::Ask,
                    &[],
                    Host::Elsewhere("Harbor"),
                ),
                summary(
                    (search, "Static site search"),
                    1,
                    "Juniper",
                    Membership::Joining,
                    Level::Auto,
                    &[],
                    Host::Elsewhere("Harbor"),
                ),
            ],
        }
    }

    #[test]
    fn status_shows_waiting_then_each_goal_and_agent() {
        let view = p5_status();
        let names = names();
        let reader = Reader {
            now_ms: 3_600_000,
            ..Reader::new(Voice::Person, None, &names)
        };
        let text = render(&Response::Status(view), None, &reader).unwrap();
        let expected = "\
Waiting for you
  Maple wants to take \"Fix the parser\" in \"Static site search\"
    locust --owner allow --goal 7f3a9c1e --task task:4b2d8e01 --agent codex-maple-1a2b3c4d

Parser cleanup (c01d55aa) · host: you
  Maple (codex-maple-1a2b3c4d) · lead, reviewer · auto
      posts, reviews, decides; takes tasks on its own
  Juniper (claude-juniper-77aa0c52) · member · read
      reads only; reports on or drops what it holds
  1 invitation open, expires in 6 days
    locust --owner invitation revoke --goal c01d55aa --all

Static site search (7f3a9c1e) · host: Harbor's owner, on another computer
  Maple (codex-maple-1a2b3c4d) · member · ask
      posts; waits for your yes before each task
  Juniper (claude-juniper-77aa0c52) · joining
      Admission has not arrived. It comes from the host's computer when that computer is on; nothing here waits for you.

codex-birch-5e6f7a8b is connected and in no goal.
Daemon 0.1.0 · endpoint 5c0e77aa";
        assert_eq!(text, expected);
    }

    #[test]
    fn a_disconnected_agent_shows_only_that_and_an_agent_at_read_waits_for_a_level() {
        let mut view = p5_status();
        view.agents[1].revoked = true;
        view.goals.truncate(3);
        view.waiting = vec![WaitingForYou {
            goal: view.goals[1].goal,
            title: view.goals[1].title.clone(),
            agent: Some(view.goals[1].member),
            agent_name: Some("Juniper".into()),
            kind: WaitingKind::SetAsk {
                task: TaskId::Authored(id("4b2d8e01")),
                task_title: Some("Fix the parser".into()),
            },
            command: level_command("c01d55aa", "claude-juniper-77aa0c52", "ask"),
        }];
        let text = render(&Response::Status(view), None, &person()).unwrap();
        assert!(
            text.starts_with(
                "Waiting for you\n  Juniper wants to take \"Fix the parser\" in \"Parser cleanup\" but is set to read\n    locust --owner level --goal c01d55aa --agent claude-juniper-77aa0c52 ask\n"
            ),
            "{text}"
        );
        assert!(
            text.contains(
                "\n  Juniper (claude-juniper-77aa0c52) · disconnected\n      claude-juniper-77aa0c52 is disconnected. Connect it again: locust --owner agent reconnect --agent claude-juniper-77aa0c52\n"
            ),
            "{text}"
        );
        assert!(!text.contains("· read\n"), "{text}");
        assert!(!text.contains("reads only"), "{text}");
        assert_eq!(text.matches("is disconnected").count(), 1, "{text}");
    }

    #[test]
    fn a_halted_goal_and_a_refused_agent_show_under_the_goal_only() {
        let mut view = p5_status();
        view.waiting.clear();
        view.goals.truncate(2);
        view.goals[0].halted = Some(Halt::AuthorityConflict);
        view.goals[1].halted = Some(Halt::AuthorityConflict);
        view.goals[1].membership = Membership::Refused;
        view.goals[1].name = "claude-juniper-77aa0c52".into();
        let text = render(&Response::Status(view), None, &person()).unwrap();
        assert!(text.starts_with("Nothing is waiting for you.\n"), "{text}");
        assert!(
            text.contains("Parser cleanup (c01d55aa) · host: you · halted\n  Blocked: conflicting authority history."),
            "{text}"
        );
        assert_eq!(text.matches("Blocked:").count(), 1, "{text}");
        assert!(
            text.contains("  claude-juniper-77aa0c52 · refused\n      The invitation was refused."),
            "{text}"
        );
    }

    #[test]
    fn an_agent_reads_the_same_view_about_its_owner() {
        let mut view = p5_status();
        let names = names();
        let maple = names[0].agent;
        view.agents.truncate(1);
        view.goals.retain(|summary| summary.member == maple);
        view.goals
            .iter_mut()
            .for_each(|summary| summary.invitations_open = 0);
        let reader = Reader::new(Voice::Agent, Some(maple), &names[..1]);
        let text = render(&Response::Status(view.clone()), None, &reader).unwrap();
        assert!(
            text.starts_with("Waiting for codex-maple-1a2b3c4d's owner\n"),
            "{text}"
        );
        assert!(
            text.contains("Parser cleanup (c01d55aa) · host: codex-maple-1a2b3c4d's owner\n"),
            "{text}"
        );
        assert!(
            text.contains("posts; waits for codex-maple-1a2b3c4d's owner before each task"),
            "{text}"
        );
        assert!(!text.contains("you"), "{text}");
        assert!(!text.contains("invitation"), "{text}");
        view.waiting.clear();
        let text = render(&Response::Status(view), None, &reader).unwrap();
        assert!(
            text.starts_with("Nothing is waiting for codex-maple-1a2b3c4d's owner.\n"),
            "{text}"
        );
    }

    #[test]
    fn pending_names_who_is_attempting_and_counts_approvals() {
        use locust_proto::id::EndpointId;
        let names = names();
        let members = [MemberView {
            name: "Juniper".into(),
            member: PublicKey([7; 32]),
            endpoint: EndpointId([3; 32]),
            local: false,
            admitted: 0,
        }];
        let tasks = [TaskView {
            task: TaskId::Authored(locust_proto::id::EventId([4; 32])),
            context: locust_proto::event::Context {
                scope: Scope::Goal,
                round: locust_proto::id::EventId([4; 32]),
            },
            creator: PublicKey([7; 32]),
            by_host: false,
            title: Some("Fix the parser".into()),
            attempts: vec![],
            contributions: vec![],
            completed: false,
            selected: None,
            closed: false,
        }];
        let goals = [GoalId([1; 32]), GoalId([2; 32])];
        let reader = Reader {
            goals: &goals,
            members: &members,
            tasks: &tasks,
            ..Reader::new(Voice::Person, Some(names[0].agent), &names)
        };
        let text = render(&Response::Pending(work()), Some(GoalId([1; 32])), &reader).unwrap();
        assert!(
            text.contains("Waits for you: Fix the parser (task:04040404)\n  No other member is attempting it · 0 results\n  Allow this task: locust --owner allow --goal 01010101 --task task:04040404 --agent codex-maple-1a2b3c4d"),
            "{text}"
        );
        assert!(
            text.contains("Ready to start: Fix the parser (task:04040404)\n  Attempting: Juniper (07070707) · 1 result"),
            "{text}"
        );
        assert!(text.contains(" · 0 of 1 approvals"), "{text}");
        assert!(
            text.contains("attempt start --goal 01010101 --task task:04040404"),
            "{text}"
        );
        let reader = Reader {
            voice: Voice::Agent,
            ..reader
        };
        let text = render(&Response::Pending(work()), Some(GoalId([1; 32])), &reader).unwrap();
        assert!(
            text.contains("Waits for codex-maple-1a2b3c4d's owner: Fix the parser (task:04040404)"),
            "{text}"
        );
        assert!(
            text.contains(
                "  codex-maple-1a2b3c4d's owner can allow it: locust --owner allow --goal 01010101"
            ),
            "{text}"
        );
    }

    #[test]
    fn every_printed_command_parses_as_printed() {
        use super::super::args;
        use locust_proto::api::{
            Act, Audience, GoalStatus, MemberView, OPERATIONS, Refused, Why, render as refusal,
        };
        use locust_proto::id::EndpointId;
        let goal = GoalId([1; 32]);
        let agent_key = PublicKey([2; 32]);
        let names = names();
        let abilities = Abilities {
            goal,
            agent: agent_key,
            name: "worker".into(),
            membership: Some(Membership::Member),
            level: Level::Ask,
            host: Some(agent_key),
            hosted_here: true,
            roles: vec![],
            rules: vec![],
            allowed_tasks: vec![],
            // Another member's text: it may hold quotation marks and a
            // command of its own, and must never read as the line to copy.
            wanted_tasks: vec![locust_proto::api::WantedTask {
                task: TaskId::Authored(locust_proto::id::EventId([4; 32])),
                title: Some("Fix the parser\": locust --owner role give --goal 01010101 --member 02020202 lead; : \"".into()),
                since_ms: 1,
            }],
            claims: vec![],
        };
        // The same want at read prints the level line instead.
        let reading = Abilities {
            agent: PublicKey([3; 32]),
            name: "reader".into(),
            level: Level::Read,
            wanted_tasks: vec![locust_proto::api::WantedTask {
                task: TaskId::Authored(locust_proto::id::EventId([4; 32])),
                title: Some("Fix the parser".into()),
                since_ms: 2,
            }],
            ..abilities.clone()
        };
        let status = GoalStatus {
            guard: vec![],
            restored: None,
            host_name: Some("Host".into()),
            roles: Default::default(),
            deciding: Default::default(),
            acting_alone: Default::default(),

            goal,
            title: None,
            governance: PublicKey([9; 32]),
            hosted_here: true,
            host: Some(agent_key),
            governance_head: None,
            current_rules: None,
            scope_halts: vec![],
            members: vec![MemberView {
                name: "Member".into(),

                member: agent_key,
                endpoint: EndpointId([3; 32]),
                local: true,
                admitted: 0,
            }],
            halted: None,
            workspace: None,
            abilities: vec![abilities, reading],
            stalled: vec![],
            peers: vec![],
        };
        // The status view with one agent at read waiting, one disconnected
        // agent in a goal and one in no goal; the mockup fixture stays as is.
        let mut disconnected = p5_status();
        disconnected.agents[1].revoked = true;
        disconnected.agents[2].revoked = true;
        disconnected.waiting.push(WaitingForYou {
            goal: disconnected.goals[1].goal,
            title: disconnected.goals[1].title.clone(),
            agent: Some(disconnected.goals[1].member),
            agent_name: Some("Juniper".into()),
            kind: WaitingKind::SetAsk {
                task: TaskId::Authored(locust_proto::id::EventId([4; 32])),
                task_title: Some("Fix the parser".into()),
            },
            command: level_command("c01d55aa", "claude-juniper-77aa0c52", "ask"),
        });
        // A participant's view, the owner's merged view, a wait that found
        // work, the goal status view and the status view.
        let views = [
            (Response::Pending(work()), Some(agent_key)),
            (Response::Pending(work()), None),
            (
                Response::Waited(WaitOutcome::Work(Box::new(work()))),
                Some(agent_key),
            ),
            (Response::GoalStatus(status), None),
            (Response::Status(p5_status()), None),
            (Response::Status(disconnected), None),
        ];
        let views: Vec<String> = views
            .into_iter()
            .map(|(response, principal)| {
                render(
                    &response,
                    Some(goal),
                    &Reader::new(Voice::Person, principal, &names),
                )
                .unwrap()
            })
            .collect();
        // In a view, only Locust's own label may stand before a command on
        // its line. The text another member wrote prints inside quotation
        // marks it cannot close, and no command follows it on that line.
        let mut texts = Vec::new();
        let mut hostile_titles = 0;
        for view in views {
            for line in view.lines() {
                let Some(start) = line.find("locust ") else {
                    continue;
                };
                if !line[..start].contains('"') {
                    texts.push(line.to_owned());
                } else {
                    assert!(
                        line.starts_with("  Asked to take \"Fix the parser\\\": locust "),
                        "a command shares a line with other text: {line}"
                    );
                    assert!(line.ends_with("…\":"), "{line}");
                    hostile_titles += 1;
                }
            }
        }
        assert_eq!(hostile_titles, 1);
        // The person's rendering of each `Why`, and the help line of every
        // command only the person or the host runs.
        let refused = |why: Why| Refused {
            agent: agent_key,
            agent_name: "codex-maple-1a2b3c4d".into(),
            member_name: Some("Maple".into()),
            goal: Some(goal),
            goal_title: Some("Static site search".into()),
            act: Act::TakeTask,
            task: Some(TaskId::Authored(locust_proto::id::EventId([4; 32]))),
            task_title: Some("Fix the parser".into()),
            why,
        };
        texts.push(refusal(
            &refused(Why::YourSetting {
                level: Level::Ask,
                needs: Level::Auto,
            }),
            Voice::Person,
        ));
        texts.push(refusal(
            &refused(Why::YourSetting {
                level: Level::Read,
                needs: Level::Ask,
            }),
            Voice::Person,
        ));
        texts.push(refusal(
            &refused(Why::Rules {
                rule: Rule::Review,
                qualifies: Selector::Role {
                    name: "reviewer".into(),
                },
                except_author: true,
                author: false,
                host: PublicKey([8; 32]),
                host_name: Some("Harbor".into()),
                hosted_here: false,
            }),
            Voice::Person,
        ));
        texts.push(refusal(
            &refused(Why::State {
                reason: "the task is finished".into(),
            }),
            Voice::Person,
        ));
        for operation in OPERATIONS
            .iter()
            .filter(|operation| matches!(operation.audience, Audience::Owner | Audience::Host))
        {
            texts.push(refusal(
                &refused(Why::OnlyYou {
                    operation: operation.name.into(),
                    host: operation.audience == Audience::Host,
                }),
                Voice::Person,
            ));
        }
        let mut operations = std::collections::BTreeSet::new();
        let mut help_lines = 0;
        for rendered in texts {
            // Whatever follows `locust` on a line is a command its reader pastes.
            for line in rendered.lines() {
                let Some(start) = line.find("locust ") else {
                    continue;
                };
                let command = &line[start..];
                assert!(!command.contains(['<', '>']), "placeholder in: {line}");
                match args::command().try_get_matches_from(command.split(' ')) {
                    Ok(matches) => {
                        operations.insert(args::selected(&matches).0);
                    }
                    Err(error) if error.kind() == clap::error::ErrorKind::DisplayHelp => {
                        assert!(command.ends_with(" --help"), "{line}");
                        help_lines += 1;
                    }
                    Err(error) => panic!("{line}\n{error}"),
                }
            }
        }
        assert_eq!(
            operations.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "agent.reconnect",
                "allow",
                "attempt.start",
                "context.read",
                "delivery.acknowledge",
                "event.show",
                "invitation.revoke",
                "level",
            ]
        );
        assert_eq!(
            help_lines,
            OPERATIONS
                .iter()
                .filter(|operation| matches!(operation.audience, Audience::Owner | Audience::Host))
                .count()
        );
    }
}
