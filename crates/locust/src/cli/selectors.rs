//! Human selectors resolve only against the authenticated, visible goal view.
//! Names and prefixes are conveniences; writes still carry exact typed identities.
use super::{LocalClient, connection, status};
use crate::failure::Failure;
use locust_proto::api::{ErrorCode, MAX_FEED_PAGE, Request, Response};
use locust_proto::event::TaskId;
use locust_proto::id::{EventId, GoalId, PublicKey};
use serde_json::{Map, Value, json};
use std::{collections::BTreeSet, path::Path};

const EVENT_FIELDS: &[&str] = &[
    "event",
    "contribution",
    "subject",
    "attempt",
    "offer",
    "cancel",
    "expected_round",
    "expected",
];

fn prefix(value: &str) -> bool {
    (8..=64).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn nonempty(value: &str, kind: &str) -> Result<(), Failure> {
    if value.trim().is_empty() {
        Err(Failure::usage(format!("{kind} selector must not be empty")))
    } else {
        Ok(())
    }
}
fn unique<T: Ord + Copy>(
    kind: &str,
    value: &str,
    candidates: impl IntoIterator<Item = T>,
) -> Result<T, Failure> {
    let candidates: BTreeSet<_> = candidates.into_iter().collect();
    let value = super::presentation::safe(value);
    match candidates.len() {
        0 => Err(Failure::new(
            ErrorCode::NotFound,
            format!("no visible {kind} matches {value}"),
        )),
        1 => Ok(*candidates.first().unwrap()),
        _ => Err(Failure::invalid(format!(
            "{kind} selector {value} is ambiguous; use a longer or full identifier"
        ))),
    }
}
pub(super) fn validate_goal(value: &str) -> Result<(), Failure> {
    nonempty(value, "goal")
}
pub(super) fn resolve_goal(
    client: &mut LocalClient,
    socket: &Path,
    value: &str,
    on_behalf: Option<PublicKey>,
) -> Result<GoalId, Failure> {
    validate_goal(value)?;
    if let Ok(id) = value.parse() {
        return Ok(id);
    }
    let known = status(client, socket, on_behalf)?;
    unique(
        "goal",
        value,
        known
            .goals
            .into_iter()
            .filter(|goal| {
                goal.title.as_deref() == Some(value)
                    || (prefix(value)
                        && goal
                            .goal
                            .to_string()
                            .starts_with(&value.to_ascii_lowercase()))
            })
            .map(|goal| goal.goal),
    )
}
fn validate_task(value: &str) -> Result<(), Failure> {
    nonempty(value, "task")?;
    if let Some((kind, id)) = value.split_once(':')
        && matches!(kind, "task" | "effect")
        && !prefix(id)
    {
        return Err(Failure::usage(
            "typed task selectors require task:<at least 8 hex> or effect:<at least 8 hex>",
        ));
    }
    Ok(())
}
pub(super) fn resolve_task(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    value: &str,
    on_behalf: Option<PublicKey>,
) -> Result<TaskId, Failure> {
    validate_task(value)?;
    if let Ok(id) = value.parse() {
        return Ok(id);
    }
    let Response::Board(tasks) = client
        .call_with(Request::Board { goal }, None, on_behalf)
        .map_err(|e| connection::client_error(e, socket))?
    else {
        unreachable!("checked response")
    };
    task_from_views(value, &tasks)
}
fn task_from_views(value: &str, tasks: &[locust_proto::api::TaskView]) -> Result<TaskId, Failure> {
    let typed = value
        .split_once(':')
        .filter(|(kind, id)| matches!(*kind, "task" | "effect") && prefix(id));
    unique(
        "task",
        value,
        tasks
            .iter()
            .filter(|task| {
                if let Some((kind, id)) = typed {
                    task.task
                        .to_string()
                        .starts_with(&format!("{kind}:{}", id.to_ascii_lowercase()))
                } else {
                    task.title.as_deref() == Some(value)
                }
            })
            .map(|task| task.task),
    )
}
pub(super) fn resolve_event(
    client: &mut LocalClient,
    socket: &Path,
    goal: GoalId,
    value: &str,
    on_behalf: Option<PublicKey>,
) -> Result<EventId, Failure> {
    if !prefix(value) {
        return Err(Failure::usage(
            "event selectors require a full identifier or at least 8 hex characters",
        ));
    }
    if let Ok(id) = value.parse() {
        return Ok(id);
    }
    let needle = value.to_ascii_lowercase();
    let mut candidates = BTreeSet::new();
    let mut after = None;
    loop {
        let Response::Events(events) = client
            .call_with(
                Request::Events {
                    goal,
                    after,
                    limit: MAX_FEED_PAGE,
                },
                None,
                on_behalf,
            )
            .map_err(|e| connection::client_error(e, socket))?
        else {
            unreachable!("checked response")
        };
        if events.is_empty() {
            break;
        }
        let next = events.iter().filter_map(|event| event.position).max();
        if next.is_none() || next <= after {
            return Err(Failure::internal("event selector feed did not advance"));
        }
        after = next;
        candidates.extend(
            events
                .into_iter()
                .filter(|event| event.event.to_string().starts_with(&needle))
                .map(|event| event.event),
        );
        if candidates.len() > 1 {
            break;
        }
    }
    unique("event", value, candidates)
}
/// Substitute valid stand-ins only while validating a named command's shape.
pub(super) fn validate_fields(fields: &mut Map<String, Value>) -> Result<(), Failure> {
    if !fields.contains_key("goal") {
        return Ok(());
    }
    for name in ["task", "parent"] {
        if let Some(Value::String(value)) = fields.get(name) {
            validate_task(value)?;
            fields.insert(name.into(), json!(TaskId::Authored(EventId([0; 32]))));
        }
    }
    for &name in EVENT_FIELDS {
        if let Some(Value::String(value)) = fields.get(name) {
            if !prefix(value) {
                return Err(Failure::usage(format!(
                    "--{} requires a full identifier or at least 8 hex characters",
                    name.replace('_', "-")
                )));
            }
            fields.insert(name.into(), json!(EventId([0; 32])));
        }
    }
    Ok(())
}
/// Call after goal and principal resolution, and only for human named commands.
pub(super) fn resolve_fields(
    client: &mut LocalClient,
    socket: &Path,
    fields: &mut Map<String, Value>,
    on_behalf: Option<PublicKey>,
) -> Result<(), Failure> {
    let Some(Value::String(goal)) = fields.get("goal") else {
        return Ok(());
    };
    let goal: GoalId = goal
        .parse()
        .map_err(|_| Failure::internal("goal must resolve before its objects"))?;
    for name in ["task", "parent"] {
        if let Some(Value::String(value)) = fields.get(name) {
            let task = resolve_task(client, socket, goal, value, on_behalf)?;
            fields.insert(name.into(), json!(task));
        }
    }
    for &name in EVENT_FIELDS {
        if let Some(Value::String(value)) = fields.get(name) {
            let event = resolve_event(client, socket, goal, value, on_behalf)?;
            fields.insert(name.into(), json!(event));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use locust_proto::api::TaskView;
    use locust_proto::event::{Context, Scope};
    use locust_proto::id::EffectId;
    fn task(task: TaskId, title: &str) -> TaskView {
        TaskView {
            task,
            context: Context {
                scope: Scope::Task(task),
                round: EventId([2; 32]),
            },
            creator: PublicKey([1; 32]),
            title: Some(title.into()),
            attempts: vec![],
            contributions: vec![],
            completed: false,
            selected: None,
            closed: false,
        }
    }
    #[test]
    fn typed_task_prefixes_keep_authored_and_derived_identities_separate() {
        let a = TaskId::Authored(EventId([0xab; 32]));
        let b = TaskId::Derived(EffectId([0xab; 32]));
        let tasks = [task(a, "Build"), task(b, "Build")];
        assert_eq!(task_from_views("task:ABABABAB", &tasks).unwrap(), a);
        assert_eq!(task_from_views("effect:abababab", &tasks).unwrap(), b);
        assert!(
            task_from_views("Build", &tasks)
                .unwrap_err()
                .message
                .contains("ambiguous")
        );
        assert!(task_from_views("abababab", &tasks).is_err());
    }
    #[test]
    fn ambiguous_prefixes_never_choose_the_first_candidate() {
        let mut second = [0xab; 32];
        second[31] = 1;
        let tasks = [
            task(TaskId::Authored(EventId([0xab; 32])), "one"),
            task(TaskId::Authored(EventId(second)), "two"),
        ];
        assert!(
            task_from_views("task:abababab", &tasks)
                .unwrap_err()
                .message
                .contains("ambiguous")
        );
        assert_eq!(task_from_views("two", &tasks).unwrap(), tasks[1].task);
        assert!(validate_task("task:abcd").is_err());
    }
    #[test]
    fn duplicate_membership_rows_do_not_make_one_goal_ambiguous() {
        assert_eq!(
            unique("goal", "demo", [GoalId([1; 32]), GoalId([1; 32])]).unwrap(),
            GoalId([1; 32])
        );
        assert!(unique("goal", "demo", [GoalId([1; 32]), GoalId([2; 32])]).is_err());
    }
}
