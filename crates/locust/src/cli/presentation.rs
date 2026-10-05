//! Human views of authoritative API responses. Labels never act as identity.

use locust_proto::api::{
    AgentView, AttentionEntry, GoalGrants, GoalPermissions, Halt, Membership, PendingWork,
    Response, SessionState, SessionView, TaskView, WaitOutcome,
};
use locust_proto::event::{Body, Scope, TaskId};
use locust_proto::id::{GoalId, PublicKey};

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
            "Blocked: conflicting authority history. Inspect the retained events with the goal administrator; decisions cannot advance."
        }
        Halt::SignerRecovery => {
            "Blocked: this daemon's signer is recovering. Restore its signed history before making further writes; reads remain available."
        }
    }
}

fn membership_action(membership: Membership) -> Option<&'static str> {
    match membership {
        Membership::Joining => Some(
            "Admission has not arrived. Check connectivity to the issuer and ask the goal administrator if admission remains pending.",
        ),
        Membership::Refused => Some(
            "The invitation was refused. Ask the goal administrator for a fresh invitation, inspect it, and join again.",
        ),
        Membership::Removed => Some(
            "This participant was removed. Ask the goal administrator about readmission; changing local permissions does not restore membership.",
        ),
        Membership::Left => Some(
            "This participant left. To participate again, obtain and inspect a fresh invitation, then join again.",
        ),
        Membership::Member => None,
    }
}

fn grant_rows(grants: GoalGrants) -> Vec<String> {
    [
        (
            "administer",
            grants.administer,
            "administer goal membership and rules",
        ),
        (
            "contribute",
            grants.contribute,
            "publish contributions and task discussion",
        ),
        (
            "execute",
            grants.execute,
            "start eligible work in this goal",
        ),
        ("review", grants.review, "record eligible reviews"),
        ("select", grants.select, "select eligible contributions"),
        ("flow", grants.flow, "open and change eligible work flow"),
        (
            "takeover",
            grants.takeover,
            "take over a claim from another session",
        ),
    ]
    .into_iter()
    .map(|(name, allowed, description)| {
        format!(
            "  {name:<11} {:<11} {description}",
            if allowed { "allowed" } else { "not allowed" }
        )
    })
    .collect()
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

fn command_context(goal: Option<GoalId>) -> String {
    let mut command = "locust".to_owned();
    if let Some(goal) = goal {
        command.push_str(&format!(" {{operation}} --goal {goal}"));
    } else {
        command.push_str(" {operation} --goal <goal>");
    }
    command
}

fn pending(
    work: &PendingWork,
    goal: Option<GoalId>,
    principal: Option<PublicKey>,
    tasks: &[TaskView],
) -> Vec<String> {
    let mut lines = vec![format!("Observed revision {}", work.revision)];
    let context = command_context(goal);
    for item in &work.to_authorize {
        lines.push(format!(
            "Permission needed: {}",
            task_label(item.task, tasks)
        ));
        let target = principal
            .map(|key| key.to_string())
            .unwrap_or_else(|| "<participant>".into());
        let goal = goal
            .map(|id| id.to_string())
            .unwrap_or_else(|| "<goal>".into());
        lines.push(format!("  Allow this task: locust --owner permission allow --goal {goal} --agent {target} --task {} execute", item.task));
    }
    for item in &work.to_start {
        lines.push(format!("Ready to start: {}", task_label(item.task, tasks)));
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
        lines.push("  Resume that session, or explicitly authorize takeover after checking its work. A held claim does not prove a process is running.".into());
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
            "Review needed: {} · {}",
            item.subject,
            scope(item.context.scope)
        ));
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
            "  Read attributed findings and reviews: {} --limit 20",
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

fn permission_view(view: &GoalPermissions) -> Vec<String> {
    let mut lines = vec![
        format!("Permissions for {} ({})", safe(&view.name), view.agent),
        format!("Goal {}", view.goal),
        format!(
            "Membership: {}",
            view.membership
                .as_ref()
                .map(tag)
                .unwrap_or_else(|| "not a member".into())
        ),
        format!(
            "Credential: {}",
            if view.revoked { "revoked" } else { "active" }
        ),
    ];
    lines.extend(grant_rows(view.grants));
    if let Some(action) = view.membership.and_then(membership_action) {
        lines.push(action.into());
    }
    for authorization in &view.task_authorizations {
        lines.push(format!(
            "Task permission: {} · round {} · execute{} · {}",
            authorization
                .task
                .map(|task| task.to_string())
                .unwrap_or_else(|| "task history unavailable".into()),
            authorization.round,
            if authorization.takeover {
                " and takeover"
            } else {
                ""
            },
            if authorization.current {
                "current round"
            } else {
                "inactive round"
            }
        ));
        if let Some(task) = authorization.task {
            lines.push(format!("  Revoke this task permission: locust --owner permission revoke --goal {} --agent {} --task {task}", view.goal, view.agent));
        }
    }
    lines.push("Membership, organization eligibility, client tool approval, and local permissions are separate.".into());
    lines.push("Permission changes affect future authorization checks. They do not stop a client process or cancel an existing attempt.".into());
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

fn inbox(entries: &[AttentionEntry]) -> Vec<String> {
    if entries.is_empty() {
        return vec!["No pending work or goal-wide halt in the local participant inbox.".into()];
    }
    let mut lines = vec!["Local participant attention".into()];
    for entry in entries {
        lines.push(format!(
            "\n{} ({}) · {} ({})",
            safe(entry.title.as_deref().unwrap_or("Title unavailable")),
            entry.goal,
            safe(&entry.name),
            entry.agent
        ));
        if let Some(reason) = entry.halted {
            lines.push(halt(reason).into());
        }
        if !entry.grants.review && !entry.pending.to_review.is_empty() {
            lines.push(format!("Review permission is missing. Allow it if intended: locust --owner permission allow --goal {} --agent {} review", entry.goal, entry.agent));
        }
        lines.extend(pending(
            &entry.pending,
            Some(entry.goal),
            Some(entry.agent),
            &entry.tasks,
        ));
    }
    lines.push("Observation only: no work or context was acknowledged.".into());
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
                lines.push(format!("Participant {} · credential {} · goal management {}", label(agent.agent, &status.agents), if agent.revoked { "revoked" } else { "active" }, if agent.grants.manage_goals { "allowed" } else { "not allowed" }));
            }
            for goal in &status.goals {
                lines.push(format!("Goal {} ({}) · {} · {}", safe(goal.title.as_deref().unwrap_or("Title unavailable")), goal.goal, label(goal.member, &status.agents), tag(&goal.membership)));
                if let Some(action) = membership_action(goal.membership) { lines.push(action.into()); }
                if let Some(reason) = goal.halted { lines.push(halt(reason).into()); }
            }
            lines
        }
        Response::GoalStatus(view) => {
            let mut lines = vec![format!("{} ({})", safe(view.title.as_deref().unwrap_or("Title unavailable")), view.goal), format!("Administrator: {}", label(view.administrator, names))];
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
            if principal.is_some() {
                lines.push("This participant's local standing permissions:".into());
                lines.extend(grant_rows(view.grants));
            } else {
                lines.push(format!("Inspect a participant's local permissions: locust --owner permission inspect --goal {} --agent <name>", view.goal));
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
                lines.push("Use pending for permission, start, review and acknowledgment actions.".into());
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
        Response::Pending(work) => pending(work, goal, principal, &[]),
        Response::Waited(WaitOutcome::Work(work)) => pending(work, goal, principal, &[]),
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
            let mut lines = events.iter().map(|event| format!("{} · {} · {} · {}", event.event, safe(&event.kind), label(event.author, names), tag(&event.standing))).collect::<Vec<_>>();
            if lines.is_empty() { lines.push("No events in this page.".into()); }
            lines
        }
        Response::Permissions(view) => permission_view(view),
        Response::Inbox(entries) => inbox(entries),
        _ => return None,
    };
    Some(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::{AgentView, Grants};

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
            grants: Grants::default(),
            revoked: false,
        }];
        assert_eq!(label(key, &names), format!("worker ({key})"));
    }

    #[test]
    fn permissions_keep_independent_rights_and_task_overrides_visible() {
        let view = GoalPermissions {
            goal: GoalId([1; 32]),
            agent: PublicKey([2; 32]),
            name: "worker".into(),
            revoked: false,
            membership: Some(locust_proto::api::Membership::Member),
            grants: GoalGrants {
                review: true,
                ..Default::default()
            },
            task_authorizations: vec![locust_proto::api::TaskAuthorization {
                task: Some(TaskId::Authored(locust_proto::id::EventId([3; 32]))),
                round: locust_proto::id::EventId([3; 32]),
                current: true,
                takeover: false,
            }],
        };
        let rendered = permission_view(&view).join("\n");
        assert!(rendered.contains("execute     not allowed"));
        assert!(rendered.contains("review      allowed"));
        assert!(rendered.contains("Task permission:"));
        assert!(rendered.contains("permission revoke"));
        assert!(rendered.contains("do not stop a client process"));
    }
}
