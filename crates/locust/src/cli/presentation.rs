//! Human views of authoritative API responses. Labels never act as identity.

use locust_proto::api::{
    Abilities, AgentView, Halt, Level, Membership, PendingWork, Response, Rule, SessionState,
    SessionView, TaskView, WaitOutcome,
};
use locust_proto::event::{Body, Scope, TaskId};
use locust_proto::id::{GoalId, PublicKey};
use locust_proto::organization::{CompletionRule, Selector};

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
        CompletionRule::All { rules } => rules
            .iter()
            .map(counts_clause)
            .collect::<Vec<_>>()
            .join(" and "),
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

/// Render untrusted text on one terminal line without terminal controls or bidi
/// overrides. No content is silently removed or shortened.
pub(super) fn safe(text: &str) -> String {
    text.chars().flat_map(|character| {
        if character.is_control()
            || matches!(character, '\u{061c}' | '\u{200b}'..='\u{200f}' | '\u{2028}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
        {
            character.escape_unicode().collect::<Vec<_>>()
        } else {
            vec![character]
        }
    }).collect()
}

fn label(key: PublicKey, names: &[AgentView]) -> String {
    names
        .iter()
        .find(|agent| agent.agent == key)
        .map(|agent| format!("{} ({key})", safe(&agent.name)))
        .unwrap_or_else(|| key.to_string())
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
            "Blocked: this daemon's signer is recovering. Restore its signed history before making further writes; reads remain available."
        }
    }
}

fn membership_action(membership: Membership) -> Option<&'static str> {
    match membership {
        Membership::Joining => Some(
            "Admission has not arrived. Check connectivity to the issuer and ask the goal host if admission remains pending.",
        ),
        Membership::Refused => Some(
            "The invitation was refused. Ask the goal host for a fresh invitation, inspect it, and join again.",
        ),
        Membership::Removed => Some(
            "This participant was removed. Ask the goal host about readmission; changing local permissions does not restore membership.",
        ),
        Membership::Left => Some(
            "This participant left. To participate again, obtain and inspect a fresh invitation, then join again.",
        ),
        Membership::Member => None,
    }
}

pub(super) fn standing_line(abilities: &Abilities) -> String {
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
    format!(
        "{}; {}",
        actions.join(", "),
        if abilities.level == Level::Ask {
            "asks before each task"
        } else {
            "takes tasks on its own"
        }
    )
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

fn task_label(task: TaskId, tasks: &[TaskView]) -> String {
    tasks
        .iter()
        .find(|view| view.task == task)
        .and_then(|view| view.title.as_deref())
        .map(|title| format!("{} ({task})", safe(title)))
        .unwrap_or_else(|| task.to_string())
}

// Every command a view prints is complete: its reader runs it as printed, with
// only their own connection flags or environment. A view that cannot name a
// value prints a command that can, never a placeholder. The test
// `every_printed_command_parses_as_printed` feeds each one to the parser.
fn command_context(goal: GoalId) -> String {
    format!("locust {{operation}} --goal {goal}")
}

fn pending(
    work: &PendingWork,
    goal: GoalId,
    principal: Option<PublicKey>,
    tasks: &[TaskView],
) -> Vec<String> {
    let mut lines = vec![format!("Observed revision {}", work.revision)];
    let context = command_context(goal);
    for item in &work.ask_first {
        lines.push(format!("Ask first: {}", task_label(item.task, tasks)));
        lines.push(format!(
            "  {} attempting · {} results",
            item.attempting.len(),
            item.results
        ));
        for attempt in &item.attempting {
            lines.push(format!(
                "  Attempting: {} · {}",
                attempt.member,
                attempt
                    .status
                    .as_ref()
                    .map(tag)
                    .unwrap_or_else(|| "no report".into())
            ));
        }
        lines.push(match principal {
            Some(agent) => format!(
                "  Allow this task: locust --owner --agent {agent} allow --goal {goal} --task {}",
                item.task
            ),
            None => "  Select an agent with --agent to see its task allowance command.".into(),
        });
    }
    for item in &work.to_start {
        lines.push(format!("Ready to start: {}", task_label(item.task, tasks)));
        lines.push(format!(
            "  {} attempting · {} results",
            item.attempting.len(),
            item.results
        ));
        for attempt in &item.attempting {
            lines.push(format!(
                "  Attempting: {} · {}",
                attempt.member,
                attempt
                    .status
                    .as_ref()
                    .map(tag)
                    .unwrap_or_else(|| "no report".into())
            ));
        }
        lines.push(format!(
            "  Participant action (requires its --session file): {} --task {}{}",
            context.replace("{operation}", "attempt start"),
            item.task,
            item.offer
                .map(|offer| format!(" --offer {offer}"))
                .unwrap_or_default()
        ));
    }
    for claim in &work.claimed {
        lines.push(format!(
            "Claim held here: {} · attempt {} · generation {}",
            task_label(claim.task, tasks),
            claim.attempt,
            claim.generation
        ));
    }
    for claim in &work.held_elsewhere {
        lines.push(format!(
            "Claim held by session {}: {} · attempt {} · generation {}",
            claim.instance,
            task_label(claim.task, tasks),
            claim.attempt,
            claim.generation
        ));
        lines.push("  Resume that session, or take over after checking its work. A held claim does not prove a process is running.".into());
    }
    for item in &work.to_acknowledge {
        lines.push(format!(
            "Cancellation needs an outcome: {} · attempt {} · cancellation {}",
            task_label(item.task, tasks),
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
                verdict.member,
                if verdict.approve { "approve" } else { "reject" },
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

pub(super) fn render(
    response: &Response,
    names: &[AgentView],
    goal: Option<GoalId>,
    principal: Option<PublicKey>,
) -> Option<String> {
    let lines = match response {
        Response::Status(status) => {
            let mut lines = vec![format!("Daemon {}", safe(&status.daemon_version))];
            if let Some(endpoint) = status.endpoint { lines.push(format!("Endpoint {endpoint}")); }
            for agent in &status.agents {
                lines.push(format!("Participant {} · credential {}", label(agent.agent, &status.agents), if agent.revoked { "revoked" } else { "active" }));
            }
            for goal in &status.goals {
                lines.push(format!("Goal {} ({}) · {} · {}", safe(goal.title.as_deref().unwrap_or("Title unavailable")), goal.goal, label(goal.member, &status.agents), tag(&goal.membership)));
                lines.push(format!("  Level: {} · {}", tag(&goal.abilities.level), standing_line(&goal.abilities)));
                if let Some(action) = membership_action(goal.membership) { lines.push(action.into()); }
                if let Some(reason) = goal.halted { lines.push(halt(reason).into()); }
            }
            lines
        }
        Response::GoalStatus(view) => {
            let mut lines = vec![format!("{} ({})", safe(view.title.as_deref().unwrap_or("Title unavailable")), view.goal), format!("Host: {}", label(view.host, names))];
            if let Some(reason) = view.halted { lines.push(halt(reason).into()); }
            for item in &view.scope_halts { lines.push(format!("{}: {}", scope(item.context.scope), halt(item.reason))); }
            for member in &view.members {
                lines.push(format!("Member: {} · {} · endpoint {}", label(member.member, names), if member.local { "local" } else { "remote" }, member.endpoint));
            }
            if let Some(rules) = view.current_rules { lines.push(format!("Rules revision: {rules}")); }
            if let Some(workspace) = &view.workspace {
                lines.push(format!("Shared workspace: {}", tag(&workspace.authority)));
                if let Some(head) = &workspace.head { lines.push(format!("Shared revision: {}", head.revision)); }
            }
            for abilities in &view.abilities {
                lines.push(format!("{} · level {} · {}", safe(&abilities.name), tag(&abilities.level), standing_line(abilities)));
                for wanted in &abilities.wanted_tasks {
                    lines.push(format!("  Asked to take \"{}\": locust --owner --agent {} allow --goal {} --task {}", safe(wanted.title.as_deref().unwrap_or("this task")), abilities.agent, view.goal, wanted.task));
                }
            }
            for stalled in &view.stalled {
                lines.push(format!("Step {} stalled for {}: {}", stalled.effect, label(stalled.runner, names), tag(&stalled.reason)));
            }
            for peer in &view.peers {
                lines.push(format!("Peer {}: {} · last successful sync {}", peer.endpoint, if peer.connected { "connected" } else { "disconnected" }, peer.last_sync_ms.map(|at| format!("{at} ms since Unix epoch")).unwrap_or_else(|| "not observed".into())));
            }
            lines.push("Peer connectivity describes this daemon's observation, not a remote process's activity.".into());
            lines
        }
        Response::Board(tasks) => {
            if tasks.is_empty() { vec!["No tasks in this goal.".into()] } else {
                let mut lines = vec![format!("{} tasks", tasks.len())];
                for task in tasks {
                    lines.push(format!("{} · {}", task_label(task.task, tasks), task_state(task)));
                    lines.push(format!("  By {} · {} attempts · {} contributions", label(task.creator, names), task.attempts.len(), task.contributions.len()));
                }
                lines.push("Use pending for start, review and acknowledgment actions.".into());
                lines
            }
        }
        Response::Task(task) => {
            let mut lines = vec![format!("{} · {}", task.view.task, task_state(&task.view)), format!("Created by {}", label(task.view.creator, names)), format!("Text: {}", task.text.as_deref().map(safe).unwrap_or_else(|| "not held locally".into())), format!("Current round: {}", task.view.context.round)];
            if let Some(parent) = task.parent { lines.push(format!("Parent: {parent}")); }
            if let Some(task_type) = &task.task_type { lines.push(format!("Task type: {}", safe(task_type))); }
            for (name, input) in &task.inputs { lines.push(format!("Input {}: {input}", safe(name))); }
            for attempt in &task.view.attempts { lines.push(format!("Attempt: {attempt}")); }
            for contribution in &task.view.contributions { lines.push(format!("Contribution: {contribution}")); }
            if let Some(selected) = task.view.selected { lines.push(format!("Selected: {selected}")); }
            lines.push(format!("Effective rules: {}", safe(&task.effective_rules_json)));
            lines
        }
        Response::Pending(work) => pending(work, goal?, principal, &[]),
        Response::Waited(WaitOutcome::Work(work)) => pending(work, goal?, principal, &[]),
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
            let mut lines = vec![format!("Contribution {} · {}", event.view.event, tag(&event.view.standing)), format!("Author: {}", label(event.view.author, names))];
            if event.view.by_owner { lines.push(if principal.is_some() { "by your owner" } else { "by you" }.into()); }
            if let Some(text) = &event.text { lines.push(format!("Summary: {}", safe(text))); }
            else if event.payload.is_some() { lines.push("Summary text is not held locally.".into()); }
            if let Some(task) = event.task { lines.push(format!("Task: {task}")); }
            for (kind, reference) in inspected.attempt.iter().map(|item| ("Attempt", item))
                .chain(inspected.task_round.iter().map(|item| ("Task round", item)))
                .chain(inspected.declared_sources.iter().map(|item| ("Declared source", item))) {
                match &reference.detail {
                    Some(detail) => {
                        lines.push(format!("{kind}: {} · {} · {} · {}", reference.event, safe(&detail.view.kind), label(detail.view.author, names), tag(&detail.view.standing)));
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
            let mut lines = vec![format!("Event {} · {} · {}", event.view.event, safe(&event.view.kind), tag(&event.view.standing)), format!("Author: {}", label(event.view.author, names))];
            if event.view.by_owner { lines.push(if principal.is_some() { "by your owner" } else { "by you" }.into()); }
            match &event.body {
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
            let mut lines = events.iter().map(|event| format!("{} · {} · {} · {}{}", event.event, safe(&event.kind), label(event.author, names), tag(&event.standing), if event.by_owner { if principal.is_some() { " · by your owner" } else { " · by you" } } else { "" })).collect::<Vec<_>>();
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
            standing_line(&abilities),
            "reads only; reports on or drops what it holds"
        );
        abilities.level = Level::Ask;
        assert_eq!(standing_line(&abilities), "posts; asks before each task");
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
            standing_line(&abilities),
            "posts, reviews; asks before each task"
        );
        abilities.level = Level::Auto;
        abilities.rules[1].eligible = true;
        assert_eq!(
            standing_line(&abilities),
            "posts, reviews, decides; takes tasks on its own"
        );
    }

    #[test]
    fn an_event_the_person_signed_reads_by_you_or_by_your_owner() {
        use locust_proto::api::{EventView, Standing};
        use locust_proto::id::EventId;
        let agent = PublicKey([2; 32]);
        let view = EventView {
            position: Some(1),
            event: EventId([3; 32]),
            author: agent,
            kind: "task_opened".into(),
            at_ms: 1,
            standing: Standing::Effective,
            by_owner: true,
        };
        let response = Response::Events(vec![view]);
        assert!(
            render(&response, &[], None, None)
                .unwrap()
                .contains("by you")
        );
        assert!(
            render(&response, &[], None, Some(agent))
                .unwrap()
                .contains("by your owner")
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
    fn terminal_controls_and_bidi_never_reach_the_terminal() {
        let text = "finding\u{1b}]52;c;secret\u{7}\r\nforged\u{202e}label";
        let rendered = safe(text);
        assert!(!rendered.chars().any(char::is_control));
        assert!(!rendered.contains('\u{202e}'));
        assert!(rendered.contains("\\u{1b}"));
        assert!(rendered.contains("secret"));
        assert_eq!(safe("ordinary café"), "ordinary café");
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
    }

    /// One of everything a view can ask its reader to do next.
    fn work() -> PendingWork {
        use locust_proto::api::{
            CancelItem, Claim, ContextNews, DeliveryItem, ReviewItem, WorkItem,
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
                task,
                offer: None,
                attempting: vec![],
                results: 0,
            }],
            to_start: vec![
                WorkItem {
                    task,
                    offer: None,
                    attempting: vec![],
                    results: 0,
                },
                WorkItem {
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

    #[test]
    fn every_printed_command_parses_as_printed() {
        use super::super::args;
        use locust_proto::api::{GoalStatus, MemberView};
        use locust_proto::id::EndpointId;
        let goal = GoalId([1; 32]);
        let agent = PublicKey([2; 32]);
        let abilities = Abilities {
            goal,
            agent,
            name: "worker".into(),
            membership: Some(Membership::Member),
            level: Level::Ask,
            host: Some(agent),
            hosted_here: true,
            roles: vec![],
            rules: vec![],
            allowed_tasks: vec![],
            wanted_tasks: vec![],
            claims: vec![],
        };
        let status = GoalStatus {
            goal,
            title: None,
            host: agent,
            governance_head: None,
            current_rules: None,
            scope_halts: vec![],
            members: vec![MemberView {
                member: agent,
                endpoint: EndpointId([3; 32]),
                local: true,
            }],
            halted: None,
            workspace: None,
            abilities: vec![abilities],
            stalled: vec![],
            peers: vec![],
        };
        // A participant's view, the owner's merged view, a wait that found
        // work, and the goal status view.
        let views = [
            (Response::Pending(work()), Some(agent)),
            (Response::Pending(work()), None),
            (
                Response::Waited(WaitOutcome::Work(Box::new(work()))),
                Some(agent),
            ),
            (Response::GoalStatus(status), None),
        ];
        let mut operations = std::collections::BTreeSet::new();
        for (response, principal) in views {
            let rendered = render(&response, &[], Some(goal), principal).unwrap();
            // Whatever follows `locust` on a line is a command its reader pastes.
            for line in rendered.lines() {
                let Some(start) = line.find("locust ") else {
                    continue;
                };
                let command = &line[start..];
                assert!(!command.contains(['<', '>']), "placeholder in: {line}");
                let matches = args::command()
                    .try_get_matches_from(command.split(' '))
                    .unwrap_or_else(|error| panic!("{line}\n{error}"));
                operations.insert(args::selected(&matches).0);
            }
        }
        assert_eq!(
            operations.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "allow",
                "attempt.start",
                "context.read",
                "delivery.acknowledge",
                "event.show",
            ]
        );
    }
}
