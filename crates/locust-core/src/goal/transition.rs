//! What one judged event does to the state: the task, result, document and
//! note rules. Each function applies the transition or says, with a short
//! static reason, why it is not allowed in the state it applies to.
//!
//! The state holds exactly what was applied before the event in canonical
//! order, so "the assignment exists" already means "its decision was applied
//! at or before this event's anchor".

use locust_proto::event::{Body, CancelOutcome, Event};
use locust_proto::id::{BlobHash, EndpointId, EventId, PublicKey};

use super::state::{
    AcceptedHead, Assignment, Cancellation, Entry, Note, Record, RecordKind, Revision, State,
    Submission, Task, TaskState, Verdict,
};

type Allowed = Result<(), &'static str>;

/// Moves the applied head to `decision`, whether or not it took effect: the
/// next decision names it either way. A removal starts the next key epoch
/// even when it removes nobody.
pub(super) fn advance(state: &mut State, decision: &Event, epoch: u32) {
    let id = decision.id();
    match &decision.header().body {
        Body::Genesis(genesis) => {
            state.coordinator = Some(genesis.coordinator);
            state.genesis = Some(id);
            state.epochs.push(id);
        }
        Body::MemberRemoved { .. } => state.epochs.push(id),
        _ => {}
    }
    state.head = Some(id);
    state.epoch = epoch;
}

/// Applies a decision of the chain that the chain itself did not exclude.
pub(super) fn decide(state: &mut State, decision: &Event) -> Allowed {
    let id = decision.id();
    match &decision.header().body {
        Body::Genesis(_) => Ok(()),
        Body::MemberAdmitted { member, endpoint } => {
            if state.is_member(member) {
                return Err("the key is already a member");
            }
            admit(state, *member, *endpoint);
            Ok(())
        }
        Body::MemberRemoved { member, .. } => {
            // The chain already refused the removal of a non-member.
            if let Some(endpoint) = state.members.remove(member) {
                unbind(state, member, &endpoint);
            }
            for assignment in &mut state.assignments {
                if assignment.assignee == *member {
                    assignment.revoked = true;
                }
            }
            state.leaves.remove(member);
            Ok(())
        }
        Body::TaskAssigned {
            task,
            assignee,
            attempt,
        } => assign(state, id, task, *assignee, *attempt),
        Body::CancelRequested { assignment } => cancel(state, id, assignment),
        Body::ResultAccepted { result, head } => accept(state, id, result, *head),
        Body::ResultRejected { result } => {
            let (at, task) = decidable(state, result)?;
            state.results[at].verdict = Some(Verdict::Rejected { decision: id });
            state.tasks[task].state = TaskState::Rejected;
            Ok(())
        }
        Body::RevisionAccepted { revision } => {
            let Some(Entry::Revision(doc, at)) = state.index.get(revision).copied() else {
                return Err("the event named is not a revision");
            };
            let document = state.documents.entry(doc).or_default();
            if document.revisions[at as usize].base != document.accepted {
                return Err("the revision was written against another accepted revision");
            }
            document.accepted = Some(*revision);
            Ok(())
        }
        _ => Err("not a decision"),
    }
}

fn admit(state: &mut State, member: PublicKey, endpoint: EndpointId) {
    if let Some(old) = state.members.insert(member, endpoint) {
        unbind(state, &member, &old);
    }
    state.endpoints.entry(endpoint).or_default().insert(member);
}

fn unbind(state: &mut State, member: &PublicKey, endpoint: &EndpointId) {
    if let Some(members) = state.endpoints.get_mut(endpoint) {
        members.remove(member);
        if members.is_empty() {
            state.endpoints.remove(endpoint);
        }
    }
}

fn task_at(state: &State, id: &EventId) -> Option<usize> {
    match state.index.get(id)? {
        Entry::Task(at) => Some(*at as usize),
        _ => None,
    }
}

fn assign(
    state: &mut State,
    id: EventId,
    task: &EventId,
    assignee: PublicKey,
    attempt: u32,
) -> Allowed {
    let at = task_at(state, task).ok_or("the event named is not a task")?;
    if !state.members.contains_key(&assignee) {
        return Err("the assignee is not a member");
    }
    let entry = &mut state.tasks[at];
    if entry.accepted.is_some() {
        return Err("the task already has an accepted result");
    }
    if entry.attempt.checked_add(1) != Some(attempt) {
        return Err("the attempt does not follow the task's current attempt");
    }
    if entry.max_attempts.is_some_and(|budget| attempt > budget) {
        return Err("the attempt exceeds the task's attempt budget");
    }
    entry.state = TaskState::Assigned;
    entry.attempt = attempt;
    entry.assignment = Some(id);
    entry.assignee = Some(assignee);
    entry.result = None;
    entry.cancel = None;
    entry.assignments.push(id);
    let index = Entry::Assignment(state.assignments.len() as u32);
    state.index.insert(id, index);
    state.assignments.push(Assignment {
        id,
        task: *task,
        assignee,
        attempt,
        revoked: false,
        records: Vec::new(),
        result: None,
        cancel: None,
    });
    Ok(())
}

fn cancel(state: &mut State, id: EventId, assignment: &EventId) -> Allowed {
    let Some(Entry::Assignment(at)) = state.index.get(assignment).copied() else {
        return Err("the event named is not an assignment");
    };
    let entry = &mut state.assignments[at as usize];
    if entry.cancel.is_some() {
        return Err("the assignment's cancellation was already requested");
    }
    let task_id = entry.task;
    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
    let task = &mut state.tasks[task];
    let current = task.assignment == Some(*assignment);
    if current && task.accepted.is_some() {
        return Err("the assignment's result was already accepted");
    }
    state.assignments[at as usize].cancel = Some(Cancellation {
        request: id,
        outcome: None,
    });
    if current {
        task.state = TaskState::CancelRequested;
        task.cancel = Some(id);
    }
    state.index.insert(id, Entry::Cancel(at));
    Ok(())
}

/// The result and task positions of a result the coordinator may decide on:
/// the latest result of its task's current assignment, not cancelled, on a
/// task that is not accepted yet.
fn decidable(state: &State, result: &EventId) -> Result<(usize, usize), &'static str> {
    let Some(Entry::Result(at)) = state.index.get(result).copied() else {
        return Err("the event named is not a result");
    };
    let submission = &state.results[at as usize];
    if submission.verdict.is_some() {
        return Err("the result was already decided");
    }
    let task = task_at(state, &submission.task).ok_or("the result has no task")?;
    let entry = &state.tasks[task];
    if entry.assignment != Some(submission.assignment) || entry.result != Some(*result) {
        return Err("not the latest result of the task's current assignment");
    }
    if entry.accepted.is_some() {
        return Err("the task already has an accepted result");
    }
    match state.assignment(&submission.assignment) {
        Some(assignment) if assignment.cancel.is_none() && !assignment.revoked => {
            Ok((at as usize, task))
        }
        _ => Err("the assignment was cancelled"),
    }
}

fn accept(state: &mut State, id: EventId, result: &EventId, head: Option<BlobHash>) -> Allowed {
    let (at, task) = decidable(state, result)?;
    if let Some(head) = head {
        let accepted = state.accepted_head();
        if accepted.is_some() && accepted != state.results[at].base {
            return Err("the result was computed against another accepted head");
        }
        state.accepted_heads.push(AcceptedHead {
            head,
            decision: id,
            result: *result,
            task: state.tasks[task].id,
        });
    }
    state.results[at].verdict = Some(Verdict::Accepted { decision: id, head });
    let entry = &mut state.tasks[task];
    entry.state = TaskState::Accepted;
    entry.accepted = Some(*result);
    entry.accepted_head = head;
    Ok(())
}

/// Applies a contribution whose author is authorized at its anchor.
pub(super) fn contribute(state: &mut State, event: &Event) -> Allowed {
    let id = event.id();
    let header = event.header();
    let author = header.author;
    match &header.body {
        Body::TaskProposed {
            input,
            depends_on,
            deadline_ms,
            max_attempts,
        } => {
            state
                .index
                .insert(id, Entry::Task(state.tasks.len() as u32));
            state.tasks.push(Task {
                id,
                proposer: author,
                input: *input,
                depends_on: depends_on.clone(),
                deadline_ms: *deadline_ms,
                max_attempts: *max_attempts,
                state: TaskState::Proposed,
                attempt: 0,
                assignment: None,
                assignee: None,
                result: None,
                cancel: None,
                assignments: Vec::new(),
                accepted: None,
                accepted_head: None,
            });
            Ok(())
        }
        Body::AssignmentAccepted { assignment } => {
            record(state, id, author, assignment, RecordKind::Accepted)
        }
        Body::AssignmentDeclined { assignment } => {
            record(state, id, author, assignment, RecordKind::Declined)
        }
        Body::Progress { assignment } => {
            record(state, id, author, assignment, RecordKind::Progress)
        }
        Body::AttemptFailed { assignment } => {
            record(state, id, author, assignment, RecordKind::Failed)
        }
        Body::ResultSubmitted {
            assignment,
            base,
            patch,
            artifacts,
        } => {
            record(state, id, author, assignment, RecordKind::Submitted)?;
            let task = state.assignment(assignment).map(|entry| entry.task);
            state
                .index
                .insert(id, Entry::Result(state.results.len() as u32));
            state.results.push(Submission {
                id,
                assignment: *assignment,
                task: task.ok_or("the assignment has no task")?,
                author,
                base: *base,
                patch: *patch,
                artifacts: artifacts.clone(),
                verdict: None,
            });
            Ok(())
        }
        Body::CancelAcknowledged { cancel, outcome } => {
            acknowledge(state, id, author, cancel, *outcome)
        }
        Body::Note { about, supersedes } => {
            if let Some(Entry::Note(at)) = supersedes.and_then(|old| state.index.get(&old)) {
                let old = &mut state.notes[*at as usize];
                if old.author == author {
                    old.superseded_by = Some(id);
                }
            }
            state
                .index
                .insert(id, Entry::Note(state.notes.len() as u32));
            state.notes.push(Note {
                id,
                author,
                about: *about,
                supersedes: *supersedes,
                at_ms: header.at_ms,
                superseded_by: None,
            });
            Ok(())
        }
        Body::Revision { doc, base } => {
            let document = state.documents.entry(*doc).or_default();
            let at = document.revisions.len() as u32;
            document.revisions.push(Revision {
                id,
                author,
                base: *base,
            });
            state.index.insert(id, Entry::Revision(*doc, at));
            Ok(())
        }
        Body::LeaveRequested => {
            state.leaves.insert(author, id);
            Ok(())
        }
        _ => Err("not a contribution"),
    }
}

/// Records an assignee's report on its assignment. The report moves the task
/// only while the assignment is the task's current one, no cancellation of
/// it was applied before, and no result of the task is accepted; otherwise
/// it stays on the assignment as evidence of what the executor did.
fn record(
    state: &mut State,
    id: EventId,
    author: PublicKey,
    assignment: &EventId,
    kind: RecordKind,
) -> Allowed {
    let Some(Entry::Assignment(at)) = state.index.get(assignment).copied() else {
        return Err("the event named is not an applied assignment");
    };
    let entry = &state.assignments[at as usize];
    if entry.assignee != author {
        return Err("the author is not the assignee");
    }
    let task = state
        .task(&entry.task)
        .ok_or("the assignment has no task")?;
    if task.assignment == Some(*assignment)
        && entry.cancel.is_none()
        && !entry.revoked
        && task.accepted.is_none()
    {
        let allowed = match kind {
            RecordKind::Accepted | RecordKind::Declined => task.state == TaskState::Assigned,
            RecordKind::Progress | RecordKind::Submitted | RecordKind::Failed => {
                task.state == TaskState::Taken
            }
            RecordKind::CancelAcknowledged(_) => true,
        };
        if !allowed {
            return Err("the report is not allowed in the current attempt state");
        }
    }
    let entry = &mut state.assignments[at as usize];
    entry.records.push(Record { event: id, kind });
    let submitted = kind == RecordKind::Submitted;
    if submitted {
        entry.result = Some(id);
    }
    let live = entry.cancel.is_none() && !entry.revoked;
    let task_id = entry.task;
    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
    let task = &mut state.tasks[task];
    if task.assignment != Some(*assignment) {
        return Ok(());
    }
    if submitted && task.accepted.is_none() {
        task.result = Some(id);
    }
    if live && task.accepted.is_none() {
        task.state = match kind {
            RecordKind::Accepted => TaskState::Taken,
            RecordKind::Progress => task.state,
            RecordKind::Declined => TaskState::Declined,
            RecordKind::Failed => TaskState::Failed,
            RecordKind::Submitted => TaskState::Submitted,
            RecordKind::CancelAcknowledged(outcome) => TaskState::Cancelled(outcome),
        };
    }
    Ok(())
}

fn acknowledge(
    state: &mut State,
    id: EventId,
    author: PublicKey,
    cancel: &EventId,
    outcome: CancelOutcome,
) -> Allowed {
    let Some(Entry::Cancel(at)) = state.index.get(cancel).copied() else {
        return Err("the event named is not an applied cancellation");
    };
    let entry = &mut state.assignments[at as usize];
    if entry.assignee != author {
        return Err("the author is not the assignee");
    }
    entry.records.push(Record {
        event: id,
        kind: RecordKind::CancelAcknowledged(outcome),
    });
    if let Some(cancellation) = &mut entry.cancel {
        cancellation.outcome = Some(outcome);
    }
    let assignment = entry.id;
    let task_id = entry.task;
    let task = task_at(state, &task_id).ok_or("the assignment has no task")?;
    let task = &mut state.tasks[task];
    if task.assignment == Some(assignment) {
        task.state = TaskState::Cancelled(outcome);
        task.cancel = None;
    }
    Ok(())
}
